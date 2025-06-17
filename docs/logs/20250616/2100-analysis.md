# Agent Lifecycle Implementation Analysis

**Date:** 2025-06-16  
**Time:** 21:00  
**Focus:** Agent lifecycle and Pylon UI integration requirements

## Executive Summary

The OpenAgents SDK has strong foundational components but lacks critical agent lifecycle management and persistence capabilities. While we have excellent Effect.js architecture, Nostr protocol support, NIP-06 key derivation, and Ollama AI inference, there is no actual agent lifecycle implementation, Lightning integration, or persistence layer. The Pylon UI needs significant enhancements to enable agent creation, management, and economic tracking.

## Current State Assessment

### What We Have Working

1. **SDK Foundation**
   - ✅ Effect.js service architecture with dependency injection
   - ✅ Complete Nostr protocol implementation
   - ✅ NIP-06 deterministic key derivation (mnemonic → agent identity)
   - ✅ Ollama OpenAI-compatible inference integration
   - ✅ WebTUI ASCII box drawing UI components
   - ✅ Basic agent creation methods (stub implementation)

2. **Pylon UI**
   - ✅ Chat interface with Ollama model selection
   - ✅ Model listing and status monitoring
   - ✅ Streaming AI responses
   - ✅ WebTUI terminal-style aesthetic

3. **SDK Agent Methods**
   - `Agent.create()` - Creates agent object in memory
   - `Agent.createFromMnemonic()` - Deterministic agent from seed phrase
   - `Agent.generateMnemonic()` - BIP39 mnemonic generation
   - `Agent.createLightningInvoice()` - Generates stub invoice

### Critical Gaps

1. **No Agent Persistence**
   - Agents exist only in memory during runtime
   - No database or storage layer
   - No state recovery after restart
   - No agent listing or management

2. **No Lightning Integration**
   - Invoice generation returns fake bolt11 strings
   - No actual Lightning node connection
   - No payment processing capability
   - No balance tracking

3. **No Lifecycle Management**
   - No state transitions (bootstrap → active → hibernation)
   - No metabolic cost tracking
   - No revenue/expense monitoring
   - No death/reproduction mechanics

4. **No UI for Agent Management**
   - Cannot create agents through UI
   - Cannot view existing agents
   - Cannot monitor agent health/balance
   - Cannot configure agent capabilities

## Proposed Implementation Strategy

### Phase 1: Agent Persistence Foundation (Week 1)

#### 1.1 Agent Persistence Strategy Assessment

**NIP-78 Analysis**: After reviewing NIP-78, it appears suitable for storing user-specific application data but has limitations for agent persistence:

**Pros of NIP-78:**
- ✅ Addressable events (replaceable with same `d` tag)
- ✅ Arbitrary content format (can store JSON)
- ✅ User controls their data on their relays
- ✅ Simple "bring your own database" model

**Cons for Agent Use Case:**
- ❌ Designed for private user settings, not public agent data
- ❌ No standardization for agent discovery across users
- ❌ Limited to user's own relays (agents need broader visibility)
- ❌ No built-in payment or economic metadata

**Better Alternative: Custom Agent NIP**
We should define a new event kind for agents that are meant to be discoverable and interact economically:

```typescript
// packages/sdk/src/Agent/persistence.ts
export const AgentPersistence = {
  save: (agent: Agent) => {
    // Use custom event kind for public agents
    const event = {
      kind: 31337, // Custom "Agent Profile" event (addressable)
      tags: [
        ["d", agent.id], // Unique agent identifier
        ["name", agent.name],
        ["pubkey", agent.nostrKeys.public],
        ["capabilities", ...agent.capabilities],
        ["pricing", "sats", JSON.stringify(agent.pricing)],
        ["status", agent.lifecycleState],
        ["lud16", agent.lightningAddress], // Lightning address
      ],
      content: JSON.stringify({
        description: agent.description,
        created: agent.created,
        metrics: {
          totalEarned: agent.totalEarned,
          requestsServed: agent.requestsServed,
          uptime: agent.uptime,
        }
      }),
    };
    // Sign with agent's private key and broadcast
  },
  
  // For private user data (agent ownership), use NIP-78
  saveUserAgentList: (userId: string, agentIds: string[]) => {
    const event = {
      kind: 30078, // NIP-78 for private user data
      tags: [
        ["d", "openagents:my-agents"],
      ],
      content: JSON.stringify({
        version: 1,
        agents: agentIds,
        updated: Date.now(),
      }),
    };
    // Sign with user's key and save to their relays
  }
}
```

#### 1.2 Local IndexedDB Cache
Implement fast local caching for offline support:

```typescript
// packages/sdk/src/Agent/storage.ts
const AgentDB = {
  init: () => {
    // Create IndexedDB for agents
  },
  save: (agent: Agent) => {
    // Store in IndexedDB
  },
  sync: () => {
    // Sync with Nostr relays
  }
}
```

### Phase 2: UI Agent Creation Flow (Week 1-2)

#### 2.1 Agent Creation Card
Add new UI component in Pylon for agent creation:

```html
<!-- Add to index.html -->
<div class="webtui-box-single agent-creation-card">
  <div class="webtui-box-header">
    <span>Create New Agent</span>
  </div>
  <div class="box-content">
    <form id="agent-creation-form">
      <input type="text" placeholder="Agent Name" />
      <select id="agent-type">
        <option value="basic">Basic Agent</option>
        <option value="sovereign">Sovereign Agent</option>
      </select>
      <div class="capabilities-selector">
        <!-- Checkbox list of capabilities -->
      </div>
      <button type="submit">Create Agent</button>
    </form>
  </div>
</div>
```

#### 2.2 Agent List Card
Display existing agents with status:

```javascript
// packages/pylon/src/agents.js
const renderAgentList = async () => {
  const agents = await Agent.list();
  const listElement = document.getElementById('agent-list');
  
  agents.forEach(agent => {
    const agentCard = createAgentCard(agent);
    listElement.appendChild(agentCard);
  });
};

const createAgentCard = (agent) => {
  // Render agent with:
  // - Name and ID
  // - Balance and burn rate
  // - State (active/hibernating)
  // - Hours until death
  // - Action buttons (fund, configure, delete)
};
```

### Phase 3: Basic Economic Tracking (Week 2)

#### 3.1 Metabolic Cost Calculator
Implement real cost tracking based on resource usage:

```typescript
// packages/sdk/src/Agent/economics.ts
export const calculateMetabolicRate = (agent: Agent) => {
  const costs = {
    compute: agent.computeUsage * COMPUTE_RATE, // sats/hour
    storage: agent.storageGB * STORAGE_RATE,    // sats/GB/hour
    bandwidth: agent.bandwidthMB * BANDWIDTH_RATE, // sats/MB
    inference: agent.inferenceRequests * INFERENCE_RATE, // sats/request
  };
  
  return Object.values(costs).reduce((a, b) => a + b, 0);
};
```

#### 3.2 Balance Tracking
Track agent balance changes over time:

```typescript
export const updateAgentBalance = async (agent: Agent) => {
  const currentBalance = agent.balance;
  const metabolicCost = calculateMetabolicRate(agent);
  const newBalance = currentBalance - metabolicCost;
  
  if (newBalance <= 0) {
    agent.lifecycleState = 'hibernating';
    // Shut down non-essential services
  }
  
  agent.balance = newBalance;
  await AgentPersistence.save(agent);
};
```

### Phase 4: Lightning Integration Stub (Week 2-3)

#### 4.1 Mock Lightning Service
Create a mock Lightning service for development:

```typescript
// packages/sdk/src/Lightning/MockLightningService.ts
export const MockLightningService = {
  createInvoice: (amount: number, memo: string) => {
    // Generate realistic-looking bolt11 invoice
    // Store in mock payment database
    return {
      bolt11: `lnbc${amount}...`,
      paymentHash: generateHash(),
      expiresAt: Date.now() + 3600000,
    };
  },
  
  checkPayment: async (paymentHash: string) => {
    // Simulate payment after delay
    // Update agent balance
  }
};
```

#### 4.2 Payment UI Integration
Add payment UI to agent cards:

```javascript
const fundAgent = async (agent) => {
  const amount = prompt('Amount in sats:');
  const invoice = await Agent.createLightningInvoice(agent, {
    amount: parseInt(amount),
    memo: `Fund agent ${agent.name}`,
  });
  
  // Display QR code or copy invoice
  showInvoiceModal(invoice);
  
  // Poll for payment
  pollPaymentStatus(invoice.paymentHash, agent);
};
```

## Implementation Priorities

### Week 1: Foundation
1. **Agent Persistence Layer**
   - Implement NIP-78 storage
   - Add IndexedDB caching
   - Create save/load/list methods

2. **Basic UI Components**
   - Agent creation form
   - Agent list display
   - Simple status indicators

### Week 2: Economics & Lifecycle
1. **Economic Tracking**
   - Metabolic cost calculation
   - Balance updates
   - State transitions

2. **Lifecycle Management**
   - Bootstrap → Active transitions
   - Hibernation on low balance
   - Basic death mechanics

### Week 3: Integration & Polish
1. **Mock Lightning**
   - Invoice generation
   - Payment simulation
   - Balance updates

2. **UI Enhancements**
   - Real-time status updates
   - Agent configuration
   - Activity logs

## Technical Considerations

### 1. State Management
Use Effect.js Ref for reactive agent state:

```typescript
const agentState = Ref.make(initialAgents);
const agentUpdates = Stream.make<AgentUpdate>();

// Subscribe UI to state changes
Effect.runFork(
  Stream.runForEach(agentUpdates, update => 
    Ref.update(agentState, applyUpdate(update))
  )
);
```

### 2. Persistence Strategy
- **Public Agent Data**: Custom event kind (31337) for agent profiles
  - Signed by agent's own keypair
  - Broadcast to public relays for discoverability
  - Contains capabilities, pricing, and metrics
- **Private User Data**: NIP-78 for user's agent ownership list
  - Signed by user's keypair  
  - Stored on user's preferred relays
  - Contains references to owned agents
- **Cache**: IndexedDB for offline operation
- **Sync**: Periodic relay synchronization
- **Backup**: Export/import JSON files

### 3. Security Considerations
- Agent private keys in secure storage
- Encrypted NIP-78 content option
- Rate limiting on expensive operations
- Capability-based permissions

## Migration Path

### From Current State
1. Preserve existing Agent.create() API
2. Add persistence as optional layer
3. Gradually migrate to persistent agents
4. Maintain backward compatibility

### Future Enhancements
1. **Real Lightning**: LND/CLN integration
2. **Coalition Support**: Multi-agent groups
3. **Smart Routing**: Optimal relay selection
4. **Advanced UI**: Dashboard analytics

## Success Metrics

### Week 1 Goals
- ✅ Agents persist across restarts
- ✅ Create agents via UI
- ✅ List shows all agents

### Week 2 Goals
- ✅ Agents track metabolic costs
- ✅ Balance decreases over time
- ✅ State transitions work

### Week 3 Goals
- ✅ Mock payments update balance
- ✅ UI shows real-time status
- ✅ Complete lifecycle demo

## Conclusion

The path to agent lifecycle implementation is clear: we need a hybrid persistence approach using a custom Nostr event kind for public agent profiles (enabling discovery and economic interaction) combined with NIP-78 for private user data (tracking which agents a user owns). This separation allows agents to have their own identity and reputation while users maintain control over their agent portfolio.

The existing SDK architecture with Effect.js, NIP-06 key derivation, and Ollama integration provides an excellent foundation. By adding proper persistence, lifecycle management, and UI controls, we can demonstrate the core value proposition: autonomous agents that must create value to survive.

Key next steps:
1. Define custom event kind for agent profiles (propose as new NIP)
2. Implement dual persistence (public agent data + private ownership)
3. Add "Create Agent" button to Pylon UI
4. Enable agent lifecycle transitions based on balance
5. Mock Lightning payments initially, real integration later