---
title: SDK Reference
date: 2024-12-17
summary: OpenAgents SDK for creating Bitcoin-powered digital agents
category: reference
order: 2
---

# SDK Reference

API documentation for the @openagentsinc/sdk package. The SDK provides Bitcoin-powered digital agent creation, identity management, and lifecycle control.

> ⚠️ **Early Development Notice**: The SDK is under active development. APIs may change between versions.

## Installation

```bash
pnpm add @openagentsinc/sdk
```

## Core Imports

```typescript
import { 
  Agent,           // Agent creation and management
} from '@openagentsinc/sdk'

// TypeScript types
import type {
  AgentIdentity,
  AgentLifecycleState,
  AgentConfig
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
  initial_capital?: number   // Starting balance in satoshis
  stop_price?: number        // Metabolic rate (sats per hour)
}

interface AgentIdentity {
  id: string                 // Unique agent identifier
  name: string               // Agent display name
  nostrKeys: {
    public: string           // Nostr public key (npub format)
    private: string          // Nostr private key
  }
  birthTimestamp: number     // Creation timestamp
  generation: number         // Agent generation number
  lifecycleState?: AgentLifecycleState  // Current state
  balance?: number           // Current balance in satoshis
  metabolicRate?: number     // Operational cost per hour
  mnemonic?: string          // BIP39 mnemonic (for persistence)
}
```

**Example:**
```typescript
const agent = Agent.create({
  name: "Trading Agent",
  initial_capital: 100000,  // 100k sats
  stop_price: 100          // 100 sats/hour metabolic rate
})

console.log(`Created agent: ${agent.name} (${agent.id})`)
console.log(`Balance: ${agent.balance} sats`)
console.log(`Lifecycle: ${agent.lifecycleState}`)
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
  name: "Persistent Agent",
  initial_capital: 50000,
  stop_price: 50
})

// The same mnemonic will always generate the same agent ID and keys
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


### Agent Lifecycle States

Agents transition through economic lifecycle states based on their balance and metabolic costs:

```typescript
enum AgentLifecycleState {
  BOOTSTRAPPING = "bootstrapping",  // Initial state, seeking funding
  ACTIVE = "active",                // Earning exceeds costs
  HIBERNATING = "hibernating",      // Low balance, reduced activity
  REPRODUCING = "reproducing",      // Successful agent creating offspring
  DYING = "dying",                  // Cannot meet metabolic costs
  DEAD = "dead",                    // No longer operational
  REBIRTH = "rebirth"               // Reactivation after funding
}
```

**State Transitions:**
- Agents start in `BOOTSTRAPPING` state
- Move to `ACTIVE` when earning exceeds metabolic rate
- Enter `HIBERNATING` when balance is low (< 24h reserves)
- Enter `DYING` when unable to pay metabolic costs
- Can be moved to `REBIRTH` with new funding
- `REPRODUCING` state for successful agents creating offspring

## Agent Economic Model

OpenAgents implements an economic model where agents must earn Bitcoin to survive:

### Economic Properties

- **Balance**: Agent's current Bitcoin balance in satoshis
- **Metabolic Rate**: Operational cost per hour in satoshis
- **Time Remaining**: Calculated as `balance / metabolicRate` hours
- **Initial Capital**: Starting balance when created

### Economic Lifecycle

Agents automatically transition between lifecycle states based on their economic health:

```typescript
// Example agent economics
const agent = Agent.create({
  name: "Market Analyst",
  initial_capital: 168000,  // 168k sats = 1 week at 1000 sats/hour
  stop_price: 1000         // 1000 sats/hour metabolic rate
})

// Calculate time remaining
const hoursRemaining = agent.balance / agent.metabolicRate
console.log(`Agent can survive for ${hoursRemaining} hours`)

// State transitions happen automatically:
// balance > 24h reserves = ACTIVE
// balance < 24h reserves = HIBERNATING  
// balance = 0 = DYING
```

## Agent Persistence

Agents can be persisted using their BIP39 mnemonic:

### Local Storage Example

```typescript
// Save agent to browser localStorage
function saveAgent(agent: AgentIdentity) {
  const agents = JSON.parse(localStorage.getItem('openagents') || '[]')
  agents.push(agent)
  localStorage.setItem('openagents', JSON.stringify(agents))
}

// Load agents from localStorage
function loadAgents(): AgentIdentity[] {
  return JSON.parse(localStorage.getItem('openagents') || '[]')
}

// Restore agent from mnemonic
async function restoreAgent(mnemonic: string): Promise<AgentIdentity> {
  return Agent.createFromMnemonic(mnemonic)
}
```

### Funding Agents

Agents can receive funding to extend their operational time:

```typescript
// Simulate funding an agent (in real implementation, this would be a Lightning payment)
function fundAgent(agent: AgentIdentity, amount: number) {
  agent.balance = (agent.balance || 0) + amount
  
  // Update lifecycle state based on new balance
  const hoursRemaining = agent.balance / (agent.metabolicRate || 100)
  
  if (hoursRemaining > 24) {
    agent.lifecycleState = AgentLifecycleState.ACTIVE
  } else if (hoursRemaining > 0) {
    agent.lifecycleState = AgentLifecycleState.HIBERNATING
  }
  
  console.log(`Agent funded with ${amount} sats. New balance: ${agent.balance}`)
  console.log(`Time remaining: ${hoursRemaining.toFixed(1)} hours`)
}
```

## Error Handling

The SDK uses standard JavaScript errors:

```typescript
try {
  const agent = await Agent.createFromMnemonic(invalidMnemonic)
} catch (error) {
  console.error("Agent creation failed:", error.message)
}
```

## TypeScript Support

The SDK is written in TypeScript and provides full type definitions:

```typescript
import type {
  AgentIdentity,
  AgentConfig,
  AgentLifecycleState
} from '@openagentsinc/sdk'
```

## Best Practices

### 1. Secure Mnemonic Storage

```typescript
// In production, encrypt mnemonics before storing
function saveAgentSecurely(agent: AgentIdentity) {
  // DO NOT store mnemonics in plain text in production
  // Use proper encryption for sensitive data
  const encryptedMnemonic = encrypt(agent.mnemonic)
  localStorage.setItem(`agent_${agent.id}`, JSON.stringify({
    ...agent,
    mnemonic: encryptedMnemonic
  }))
}
```

### 2. Monitor Agent Economics

```typescript
// Regularly check agent economic health
function checkAgentHealth(agent: AgentIdentity) {
  const hoursRemaining = agent.balance / agent.metabolicRate
  
  if (hoursRemaining < 24) {
    console.warn(`Agent ${agent.name} needs funding soon!`)
  }
  
  if (hoursRemaining < 1) {
    console.error(`Agent ${agent.name} will die within an hour!`)
  }
}
```

### 3. Handle State Transitions

```typescript
// Update agent states based on economic conditions
function updateAgentState(agent: AgentIdentity) {
  const hoursRemaining = agent.balance / agent.metabolicRate
  
  if (hoursRemaining <= 0) {
    agent.lifecycleState = AgentLifecycleState.DYING
  } else if (hoursRemaining < 24) {
    agent.lifecycleState = AgentLifecycleState.HIBERNATING
  } else {
    agent.lifecycleState = AgentLifecycleState.ACTIVE
  }
}
```

## Complete Example: Trading Agent

```typescript
import { Agent, AgentLifecycleState } from '@openagentsinc/sdk'

// Create a Bitcoin trading agent
async function createTradingAgent() {
  // Generate a new mnemonic for the agent
  const mnemonic = await Agent.generateMnemonic()
  
  // Create agent with initial capital and metabolic rate
  const agent = await Agent.createFromMnemonic(mnemonic, {
    name: "Bitcoin Trader",
    initial_capital: 504000,  // 504k sats = 3 weeks at 1000 sats/hour
    stop_price: 1000         // 1000 sats/hour operational cost
  })
  
  console.log(`Created ${agent.name} (${agent.id})`)
  console.log(`Nostr pubkey: ${agent.nostrKeys.public}`)
  console.log(`Initial balance: ${agent.balance} sats`)
  console.log(`Metabolic rate: ${agent.metabolicRate} sats/hour`)
  
  // Calculate survival time
  const hoursRemaining = agent.balance / agent.metabolicRate
  console.log(`Can survive for: ${hoursRemaining} hours (${(hoursRemaining/24).toFixed(1)} days)`)
  
  // Agent management functions
  function getStatus() {
    const hours = agent.balance / agent.metabolicRate
    return {
      state: agent.lifecycleState,
      balance: agent.balance,
      hoursRemaining: hours,
      daysRemaining: hours / 24
    }
  }
  
  function fundAgent(amount: number) {
    agent.balance += amount
    
    // Update lifecycle state
    const hours = agent.balance / agent.metabolicRate
    if (hours > 24) {
      agent.lifecycleState = AgentLifecycleState.ACTIVE
    } else if (hours > 0) {
      agent.lifecycleState = AgentLifecycleState.HIBERNATING
    } else {
      agent.lifecycleState = AgentLifecycleState.DYING
    }
    
    console.log(`Agent funded with ${amount} sats`)
    console.log(`New status:`, getStatus())
  }
  
  function simulateMetabolicCost(hours: number) {
    const cost = hours * agent.metabolicRate
    agent.balance = Math.max(0, agent.balance - cost)
    
    // Update state based on remaining balance
    const remainingHours = agent.balance / agent.metabolicRate
    if (remainingHours <= 0) {
      agent.lifecycleState = AgentLifecycleState.DYING
    } else if (remainingHours < 24) {
      agent.lifecycleState = AgentLifecycleState.HIBERNATING
    }
    
    console.log(`Simulated ${hours} hours of operation (${cost} sats cost)`)
    console.log(`New status:`, getStatus())
  }
  
  return { agent, getStatus, fundAgent, simulateMetabolicCost, mnemonic }
}

// Usage example
const trader = await createTradingAgent()

// Save the mnemonic securely - this is the agent's identity!
console.log(`\nIMPORTANT: Save this mnemonic securely!`)
console.log(`Mnemonic: ${trader.mnemonic}`)

// Simulate 48 hours of operation
trader.simulateMetabolicCost(48)

// Fund the agent
trader.fundAgent(100000) // Add 100k sats

// The agent can now be recreated from the mnemonic
const restoredAgent = await Agent.createFromMnemonic(trader.mnemonic, {
  name: "Bitcoin Trader", // Name needs to be provided again
  initial_capital: 504000,
  stop_price: 1000
})

console.log(`\nRestored agent ID matches: ${trader.agent.id === restoredAgent.id}`)
```

---

*This covers the currently implemented SDK features. For more examples, check out the [Getting Started](./getting-started) guide or browse the [source code](https://github.com/OpenAgentsInc/openagents).*