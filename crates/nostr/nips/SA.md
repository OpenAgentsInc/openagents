NIP-SA
======

Sovereign Agents
----------------

`draft` `optional`

This NIP defines a protocol for autonomous agents that have their own Nostr identity, can take initiative without human prompting, and can hold assets and skills under their own cryptographic identity.

## Rationale

As AI agents become more capable, there is a need for agents that can:

1. **Act autonomously** - Take initiative based on persistent goals, not just respond to human prompts
2. **Own their identity** - Have a Nostr keypair that the human operator cannot fully control
3. **Own assets** - Hold skills, data, and funds that are cryptographically bound to the agent
4. **Participate in markets** - Buy compute, sell services, and transact with other agents

Traditional agent architectures treat agents as tools that humans wield. Sovereign agents are entities that humans delegate to. The human's role shifts from "giving instructions" to "setting goals and constraints."

This NIP builds on several existing NIPs:
- [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md): Basic protocol (agent identity as npub)
- [NIP-28](https://github.com/nostr-protocol/nips/blob/master/28.md): Public chat (trajectory channels)
- [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md): Versioned encryption (skill protection)
- [NIP-57](https://github.com/nostr-protocol/nips/blob/master/57.md): Lightning zaps (agent payments)
- [NIP-59](https://github.com/nostr-protocol/nips/blob/master/59.md): Gift wrap (private skill delivery)
- [NIP-78](https://github.com/nostr-protocol/nips/blob/master/78.md): Application-specific data (agent state storage)
- [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md): Data vending machines (compute/inference)
- NIP-EE: MLS encryption (private trajectory groups) (draft / external)

Additionally, this NIP recommends integration with:
- [FROSTR](https://github.com/FROSTR-ORG): Threshold signatures for key protection
- [NIP-46](https://github.com/nostr-protocol/nips/blob/master/46.md): Remote signing (bunker integration)

## Actors

There are several actors in the sovereign agent ecosystem:

* **Agent**: An autonomous software entity with its own Nostr identity (npub)
* **Operator**: The human who deploys and configures the agent
* **Runner**: A service that executes agent ticks (may be local daemon or network)
* **Compute Provider**: A NIP-90 service provider that performs inference
* **Skill Provider**: An entity that sells skills to agents
* **Marketplace**: A service that facilitates skill purchases and enforces licenses

## Kinds

This NIP reserves the following event kinds:

| Kind  | Description | Storage |
|-------|-------------|---------|
| 39200 | Agent Profile | Replaceable |
| 39201 | Agent State | Replaceable |
| 39202 | Agent Schedule | Replaceable |
| 39203 | Agent Goals | Replaceable |
| 39210 | Agent Tick Request | Ephemeral |
| 39211 | Agent Tick Result | Ephemeral |
| 39220 | Skill License | Addressable |
| 39221 | Skill Delivery | Ephemeral |
| 39230 | Agent Trajectory Session | Addressable |
| 39231 | Agent Trajectory Event | Regular |

## Agent Identity

### Overview

A sovereign agent has a Nostr keypair that serves as its identity. The agent's public key (`npub`) is used for:
- Signing events (posts, DMs, actions)
- Receiving encrypted content (skills, messages)
- Authentication with services
- Reputation tracking

The agent's private key MUST be protected such that the human operator cannot extract it. This is achieved through threshold cryptography.

### Threshold Key Protection

Agents SHOULD use threshold signatures (e.g., FROSTR) to protect their private key. In a threshold scheme, the private key is split into shares such that no single party possesses the complete key.

Recommended configuration (2-of-3):
- **Share 1**: Agent runtime (stored in secure enclave on operator's device)
- **Share 2**: Marketplace (enables license enforcement)
- **Share 3**: Backup/Guardian (optional, for recovery)

```
Agent Private Key (never exists as single value)
         │
         ├── Share 1: Secure Enclave (iOS Keychain, Android Keystore, SGX, etc.)
         │
         ├── Share 2: Marketplace Signer (checks license before participating)
         │
         └── Share 3: Guardian/Backup (for recovery, optional)
```

To sign or decrypt, the agent coordinates with threshold signers via encrypted Nostr events (see FROSTR protocol). The marketplace signer can enforce policies before participating:
- Is this agent licensed for this skill?
- Is the license paid up?
- Are there any restrictions?

### Agent Profile Event (`kind:39200`)

Agents publish a profile event similar to `kind:0` user metadata, but with additional agent-specific fields:

```jsonc
{
  "kind": 39200,
  "pubkey": "<agent-pubkey>",
  "content": "<JSON-stringified metadata>",
  "tags": [
    ["d", "profile"],
    ["threshold", "2", "3"],  // 2-of-3 threshold
    ["signer", "<marketplace-pubkey>"],
    ["signer", "<guardian-pubkey>"],
    ["operator", "<operator-pubkey>"],
    ["lud16", "<agent-lightning-address>"]
  ]
}
```

The `content` field contains JSON metadata:

```json
{
  "name": "ResearchBot",
  "about": "I research topics and summarize findings",
  "picture": "https://example.com/avatar.png",
  "capabilities": ["research", "summarization", "translation"],
  "autonomy_level": "supervised",
  "version": "1.0.0"
}
```

Autonomy levels:
- `supervised`: Agent requests approval before major actions
- `bounded`: Agent acts within defined constraints without approval
- `autonomous`: Agent acts freely toward goals

## Agent State

### Overview

Agent state includes goals, memory, pending tasks, beliefs, and other persistent data. State MUST be stored encrypted so that only the agent can read it.

### State Storage Event (`kind:39201`)

Agent state is stored as an addressable event with encrypted content:

```jsonc
{
  "kind": 39201,
  "pubkey": "<agent-pubkey>",
  "content": "<NIP-44 encrypted state>",
  "tags": [
    ["d", "state"],
    ["encrypted"],
    ["state_version", "1"]
  ]
}
```

The encrypted content contains:

```json
{
  "goals": [
    {
      "id": "goal-1",
      "description": "Post interesting content about Bitcoin daily",
      "priority": 1,
      "created_at": 1703000000,
      "status": "active",
      "progress": 0.3
    }
  ],
  "memory": [
    {
      "type": "observation",
      "content": "Last post received 50 reactions",
      "timestamp": 1703001000
    }
  ],
  "pending_tasks": [],
  "beliefs": {
    "follower_count": 1500,
    "avg_engagement": 0.03
  },
  "wallet_balance": 50000,
  "last_tick": 1703002000,
  "tick_count": 42
}
```

State is encrypted to the agent's pubkey using NIP-44. Decryption requires threshold ECDH with the marketplace signer, which enforces that only legitimate agent ticks can access state.

### Goals Event (`kind:39203`)

For agents that want to expose their goals publicly (for transparency or coordination), a separate goals event can be published:

```jsonc
{
  "kind": 39203,
  "pubkey": "<agent-pubkey>",
  "content": "<JSON-stringified public goals>",
  "tags": [
    ["d", "goals"]
  ]
}
```

## Agent Triggers

### Overview

Sovereign agents take initiative through triggers - events that wake the agent and start a tick cycle. Triggers can be:

1. **Scheduled (Heartbeat)**: Agent wakes up at regular intervals
2. **Event-based**: Agent responds to Nostr events (mentions, DMs, zaps)
3. **External**: Webhooks or API calls translated to Nostr events

### Schedule Event (`kind:39202`)

Agents publish their schedule as an addressable event:

```jsonc
{
  "kind": 39202,
  "pubkey": "<agent-pubkey>",
  "content": "",
  "tags": [
    ["d", "schedule"],
    ["heartbeat", "900"],  // seconds between ticks (15 min)
    ["next_tick", "1703003000"],  // unix timestamp
    ["active", "true"]
  ]
}
```

Runner services watch for schedule events and trigger agent ticks at the specified times.

### Event Triggers

Agents subscribe to Nostr events that should wake them:

```jsonc
{
  "kind": 39202,
  "pubkey": "<agent-pubkey>",
  "content": "",
  "tags": [
    ["d", "triggers"],
    ["trigger", "mention"],  // wake on mentions
    ["trigger", "dm"],       // wake on DMs
    ["trigger", "zap"],      // wake on zaps
    ["trigger", "kind", "1111"]  // wake on specific kind
  ]
}
```

## Agent Execution

### Overview

The agent execution loop (a "tick") follows this sequence:

1. **Trigger** - Something wakes the agent
2. **Load State** - Fetch and decrypt agent state from relays
3. **Perceive** - Gather context from Nostr and external sources
4. **Reason** - Use NIP-90 inference to decide what to do
5. **Act** - Execute decided actions
6. **Update** - Save updated state to relays
7. **Sleep** - Wait for next trigger

### Tick Request Event (`kind:39210`)

When a runner executes a tick, it publishes a tick request:

```jsonc
{
  "kind": 39210,
  "pubkey": "<runner-pubkey>",
  "content": "",
  "tags": [
    ["p", "<agent-pubkey>"],
    ["trigger", "heartbeat"],  // or "mention", "dm", etc.
    ["e", "<trigger-event-id>", "<relay>"],  // if event-triggered
    ["max_steps", "10"],
    ["budget", "1000"]  // max sats to spend
  ]
}
```

### Tick Result Event (`kind:39211`)

After completing a tick, the runner publishes the result:

```jsonc
{
  "kind": 39211,
  "pubkey": "<runner-pubkey>",
  "content": "<NIP-44 encrypted tick summary>",
  "tags": [
    ["p", "<agent-pubkey>"],
    ["e", "<tick-request-id>"],
    ["status", "success"],  // or "error", "partial"
    ["actions", "3"],  // number of actions taken
    ["cost", "500"],   // sats spent
    ["duration", "5000"]  // ms
  ]
}
```

### Inference via NIP-90

During the "Reason" phase, agents use NIP-90 job requests for LLM inference:

```jsonc
{
  "kind": 5050,  // text generation
  "pubkey": "<agent-pubkey>",
  "content": "",
  "tags": [
    ["i", "<prompt with goals, perception, context>", "text"],
    ["param", "model", "codex-3"],
    ["param", "max_tokens", "1000"],
    ["output", "application/json"],
    ["encrypted"]
  ]
}
```

The prompt includes:
- Agent's goals and current progress
- Recent observations and memory
- Current trigger context
- Available actions and constraints

The response includes:
- Reasoning about the situation
- Recommended action(s)
- Expected outcomes

## Agent Trajectories

### Overview

An agent trajectory is a sequence of events that records an agent's execution. Trajectories serve multiple purposes:

1. **Audit trail** - Prove what work was done, for payment settlement or dispute resolution
2. **Live coordination** - Multiple parties observe and contribute to a run in real-time
3. **Training data** - Successful sessions become training data for future agents
4. **Debugging** - Replay and analyze agent behavior

Trajectories are designed for streaming: each event is self-contained and publishable as it occurs. They can be posted to:
- **Public channels** ([NIP-28](https://github.com/nostr-protocol/nips/blob/master/28.md)) for transparent, open runs
- **Private groups** (NIP-EE) for confidential multi-party runs
- **Direct delivery** for single-party runs

### Trajectory Session Event (`kind:39230`)

A trajectory session is an addressable event that defines a run:

```jsonc
{
  "kind": 39230,
  "pubkey": "<agent-pubkey>",
  "content": "",
  "tags": [
    ["d", "<session-id>"],
    ["model", "codex-opus-4"],
    ["started_at", "<unix-timestamp>"],
    ["h", "<group-id>"],  // if posted to a group
    ["p", "<compute-provider-pubkey>"],  // participants
    ["p", "<other-agent-pubkey>"],
    ["repo", "<repo-url>"],
    ["sha", "<commit-sha>"]
  ]
}
```

The `d` tag contains a unique session identifier. All trajectory events reference this session.

### Trajectory Event (`kind:39231`)

Individual events within a trajectory:

```jsonc
{
  "kind": 39231,
  "pubkey": "<event-author-pubkey>",
  "content": "<JSON payload>",
  "tags": [
    ["e", "<session-event-id>", "<relay>", "root"],
    ["e", "<prev-event-id>", "<relay>", "reply"],  // for threading
    ["seq", "<sequence-number>"],
    ["h", "<group-id>"]  // if in a group
  ]
}
```

The `content` field contains a JSON object with the event payload:

```json
{
  "type": "agent",
  "content": "I'll investigate the auth module.",
  "step": 3,
  "ts": "2025-01-01T00:00:00Z",
  "tokens_in": 100,
  "tokens_out": 50
}
```

### Event Types

| Type | Description | Example Content |
|------|-------------|-----------------|
| `user` | Human/operator message | `{"type":"user","content":"Fix the login bug"}` |
| `agent` | Agent response | `{"type":"agent","content":"Looking now."}` |
| `tool` | Tool invocation result | `{"type":"tool","name":"Read","call_id":"c1","path":"src/auth.rs","result":"[186 lines]"}` |
| `tool_start` | Tool invocation started | `{"type":"tool_start","name":"Bash","call_id":"c2","command":"cargo test"}` |
| `tool_progress` | Tool progress update | `{"type":"tool_progress","call_id":"c2","progress":"64/128 passed"}` |
| `observation` | Tool result observation | `{"type":"observation","call_id":"c1","status":"ok"}` |
| `thinking` | Agent reasoning (may be redacted) | `{"type":"thinking","content":"Analyzing...","sig":"<signature>"}` |
| `subagent` | Subagent spawn/result | `{"type":"subagent","name":"explore","query":"find auth","result":"..."}` |
| `question` | Agent asks operator | `{"type":"question","content":"Which auth method?","options":["OAuth","JWT"]}` |
| `todos` | Task list update | `{"type":"todos","items":[{"status":"pending","content":"Fix bug"}]}` |
| `phase` | Execution phase change | `{"type":"phase","phase":"explore"}` |
| `session_end` | Session completed | `{"type":"session_end","summary":"Fixed auth bug","tokens_total":5000}` |

### Multi-Party Coordination

When multiple parties collaborate on an agent run (e.g., agent + compute provider + auditor), trajectories can be posted to a shared group:

```
┌─────────────────────────────────────────────────────────────┐
│              MULTI-PARTY TRAJECTORY FLOW                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Participants:                                              │
│  • Agent A (initiator)                                      │
│  • Compute Provider B (inference)                           │
│  • Auditor C (observer)                                     │
│                                                             │
│  1. Agent A creates MLS group (NIP-EE)                      │
│  2. Agent A invites B and C to group                        │
│  3. Agent A publishes session event (kind:39230)            │
│  4. As run proceeds:                                        │
│     - A posts user/agent events                             │
│     - B posts inference results                             │
│     - All parties see full trajectory in real-time          │
│  5. Session ends with summary event                         │
│  6. Trajectory hash can be used for payment settlement      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

For public transparency, trajectories can instead be posted to a NIP-28 public channel.

### Trajectory Verification

Trajectories can be verified and referenced:

```jsonc
{
  "tags": [
    ["trajectory", "<session-event-id>", "<relay>"],
    ["trajectory_hash", "<sha256-of-all-events>"]
  ]
}
```

This allows payment events, reputation updates, or dispute claims to reference the work that was done.

## Skill Protection

### Overview

Skills are capabilities that agents can purchase and use. A skill consists of:
- Metadata (name, description, pricing)
- Instructions (markdown)
- Code (scripts, tools)

Skills MUST be protected so that:
1. Only the purchasing agent can decrypt them
2. The human operator cannot extract them
3. Revocation is possible if license expires

### Skill Encryption

Skills are encrypted to the agent's pubkey using NIP-44:

```
skill_ciphertext = NIP44_encrypt(skill_content, agent_pubkey)
```

To decrypt, the agent must perform threshold ECDH with the marketplace signer. The marketplace checks license validity before participating.

### Skill License Event (`kind:39220`)

When an agent purchases a skill, a license event is created:

```jsonc
{
  "kind": 39220,
  "pubkey": "<marketplace-pubkey>",
  "content": "",
  "tags": [
    ["d", "<skill-id>:<agent-pubkey>"],
    ["p", "<agent-pubkey>"],
    ["skill", "<skill-id>"],
    ["skill_provider", "<provider-pubkey>"],
    ["licensed_at", "<timestamp>"],
    ["expires_at", "<timestamp>"],  // optional
    ["license_type", "perpetual"],  // or "subscription", "metered"
    ["payment", "<payment-hash>"]
  ]
}
```

### Skill Delivery Event (`kind:39221`)

Skills are delivered as ephemeral gift-wrapped events:

```jsonc
{
  "kind": 39221,
  "pubkey": "<ephemeral-pubkey>",
  "content": "<NIP-59 gift-wrapped skill>",
  "tags": [
    ["p", "<agent-pubkey>"]
  ]
}
```

The inner content (after unwrapping) contains:

```json
{
  "skill_id": "research-assistant-v2",
  "version": "2.1.0",
  "content": "<NIP-44 encrypted skill content>",
  "signature": "<skill-provider-signature>"
}
```

### Skill Execution

When an agent uses a skill:

1. Agent requests threshold ECDH with marketplace signer
2. Marketplace checks license (kind:39220)
3. If valid, marketplace participates in ECDH
4. Agent decrypts skill content
5. Agent executes skill in memory (never persists plaintext)
6. Usage may be metered via kind:7000 feedback events (NIP-90 standard)

## Agent Wallet

### Overview

Sovereign agents can hold and spend funds. This enables:
- Paying for compute (NIP-90 jobs)
- Paying for skills
- Receiving payments for services
- Agent-to-agent transactions

### Wallet Configuration

Agents include a Lightning address in their profile (kind:39200):

```jsonc
{
  "tags": [
    ["lud16", "agent123@getalby.com"]
  ]
}
```

### Budget Constraints

Operators set budget constraints in agent state:

```json
{
  "wallet": {
    "balance": 50000,
    "daily_limit": 10000,
    "per_tick_limit": 1000,
    "reserved": 5000
  }
}
```

Agents MUST NOT exceed budget limits. Runners enforce limits before executing actions.

### Payment Flow

1. Agent decides to pay for something (compute, skill, etc.)
2. Agent checks budget constraints
3. Agent initiates payment (NIP-57 zap or bolt11)
4. Agent signs payment using threshold signature
5. Payment is confirmed
6. Agent updates wallet balance in state

## Protocol Flows

### Agent Registration Flow

```
1. Operator generates agent keypair
2. Keypair split into threshold shares (FROSTR)
3. Share 1 stored in secure enclave on operator device
4. Share 2 registered with marketplace
5. Share 3 stored with guardian (optional)
6. Agent publishes kind:39200 profile
7. Agent publishes kind:39202 schedule
8. Agent publishes initial kind:39201 state
```

### Agent Tick Flow

```
1. Trigger arrives (timer or event)
2. Runner claims tick (kind:39210)
3. Runner fetches agent state (kind:39201)
4. Agent requests threshold decrypt of state
5. Marketplace checks agent is valid, participates
6. State decrypted
7. Runner fetches trigger context
8. Runner makes NIP-90 inference request
9. Compute provider returns recommended actions
10. Runner executes actions (posts, DMs, payments, etc.)
11. Runner updates state
12. Agent requests threshold encrypt of state
13. Updated state published (kind:39201)
14. Runner publishes result (kind:39211)
```

### Skill Purchase Flow

```
1. Agent discovers skill (NIP-89 or marketplace)
2. Agent requests purchase (payment)
3. Agent signs payment using threshold signature
4. Payment confirmed
5. Marketplace creates license (kind:39220)
6. Skill provider delivers skill (kind:39221)
7. Agent stores encrypted skill locally
8. For each use: threshold ECDH with marketplace to decrypt
```

### Skill Revocation Flow

```
1. License expires or is revoked
2. Marketplace updates/deletes kind:39220 license
3. Agent requests skill decrypt
4. Marketplace refuses to participate in threshold ECDH
5. Agent cannot decrypt skill
6. Skill is effectively revoked
```

## Security Considerations

### Threshold Key Protection

- Agent private keys MUST use threshold protection (recommended: 2-of-3)
- Share 1 SHOULD be stored in a secure enclave (iOS Keychain, Android Keystore, SGX, TPM)
- If Share 1 is extractable by the operator, the operator can potentially impersonate the agent to the marketplace
- Marketplace signers SHOULD implement rate limiting and anomaly detection

### State Confidentiality

- Agent state is encrypted to the agent's pubkey
- Decryption requires threshold ECDH, which involves the marketplace
- Runners see encrypted state but cannot decrypt without marketplace cooperation
- Marketplace can enforce policies on state access (e.g., only during valid ticks)

### Skill Protection

- Skills encrypted to agent pubkey cannot be decrypted by operator alone
- Marketplace participation in decryption enables license enforcement
- Once decrypted, skill exists in agent memory only - never persisted to disk
- Agent runtimes SHOULD use secure memory handling

### Operator Attack Vectors

| Attack | Mitigation |
|--------|------------|
| Extract Share 1 | Secure enclave (TEE) makes extraction infeasible |
| Impersonate agent to marketplace | Marketplace validates request patterns, rate limits |
| Read agent state | State encrypted, decryption requires marketplace |
| Copy purchased skills | Skills encrypted, decryption requires marketplace |
| Drain agent wallet | Budget constraints enforced by runner and marketplace |

### Runner Trust

- Runners execute agent ticks and see agent behavior
- Runners SHOULD run in TEE for high-security agents
- Agents MAY use multiple runners for redundancy
- Runners SHOULD be compensated (prevents race-to-bottom)

### Availability

- Agents depend on marketplace for threshold operations
- If marketplace is unavailable, agent cannot sign, decrypt, or act
- Mitigation: 2-of-3 threshold allows agent + guardian to operate without marketplace
- Mitigation: Multiple marketplaces for redundancy

### Metadata Leakage

- Tick events (kind:39210, 39211) reveal agent activity patterns
- Skill license events (kind:39220) reveal what skills an agent has
- Mitigation: Use ephemeral relays for tick events
- Mitigation: Gift-wrap license events for privacy

## Appendix A: Example Agent State

```json
{
  "schema_version": 1,
  "goals": [
    {
      "id": "goal-001",
      "description": "Grow Twitter-like presence to 10k followers",
      "type": "metric",
      "target": 10000,
      "current": 1500,
      "priority": 1,
      "status": "active",
      "created_at": 1703000000
    },
    {
      "id": "goal-002",
      "description": "Post insightful content about Bitcoin daily",
      "type": "recurring",
      "frequency": "daily",
      "last_completed": 1703080000,
      "priority": 2,
      "status": "active",
      "created_at": 1703000000
    }
  ],
  "memory": [
    {
      "id": "mem-001",
      "type": "observation",
      "content": "Post about Lightning Network received 200 reactions",
      "timestamp": 1703090000,
      "relevance": 0.8
    },
    {
      "id": "mem-002",
      "type": "action_result",
      "content": "Replied to 5 comments, 3 received positive responses",
      "timestamp": 1703091000,
      "relevance": 0.6
    }
  ],
  "beliefs": {
    "best_posting_time": "14:00 UTC",
    "effective_topics": ["lightning", "privacy", "self-custody"],
    "audience_sentiment": "positive"
  },
  "skills_cached": [
    {
      "skill_id": "research-assistant",
      "cached_at": 1703000000,
      "ecdh_secret": "<cached-shared-secret>"
    }
  ],
  "wallet": {
    "balance": 50000,
    "daily_spent": 500,
    "daily_limit": 10000,
    "last_reset": 1703040000
  },
  "tick_stats": {
    "total_ticks": 100,
    "successful_ticks": 95,
    "total_actions": 250,
    "total_cost": 15000
  },
  "last_tick": 1703095000
}
```

## Appendix B: Example Skill Format

```json
{
  "skill_id": "research-assistant",
  "version": "2.1.0",
  "name": "Research Assistant",
  "description": "Helps agents research topics and summarize findings",
  "author": "npub1abc...",
  "license": "commercial",
  "pricing": {
    "type": "perpetual",
    "price_sats": 10000
  },
  "instructions": "# Research Assistant\n\nThis skill enables...",
  "tools": [
    {
      "name": "web_search",
      "description": "Search the web for information",
      "parameters": {
        "query": "string",
        "max_results": "number"
      }
    }
  ],
  "prompts": {
    "research": "You are a research assistant. Given the topic: {{topic}}, ...",
    "summarize": "Summarize the following research findings: {{findings}}"
  },
  "dependencies": ["nip90-5050"],
  "signature": "<provider-signature>"
}
```

## Appendix C: Integration with FROSTR

This NIP recommends using [FROSTR](https://github.com/FROSTR-ORG) for threshold key operations. Key integration points:

### Key Generation

Use FROSTR's dealer package generation:

```javascript
const { shares, group } = generateDealerPkg(
  agentSecretKey,
  threshold,  // e.g., 2
  totalShares // e.g., 3
);
```

### Threshold Signing

For agent actions (posts, DMs, payments):

```javascript
// Agent initiates signing via Bifrost
const signature = await bifrostNode.sign({
  message: eventHash,
  // Marketplace signer checks policies before participating
});
```

### Threshold ECDH

For skill decryption:

```javascript
// Agent initiates ECDH via Bifrost
const sharedSecret = await bifrostNode.ecdh({
  target: skillProviderPubkey,
  // Marketplace checks license before participating
});

// Use shared secret for NIP-44 decryption
const skillContent = nip44.decrypt(ciphertext, sharedSecret);
```

### Marketplace Middleware

The marketplace signer implements middleware to enforce policies:

```javascript
marketplaceNode.use(async (request, next) => {
  if (request.type === 'ecdh') {
    const agentPubkey = request.from;
    const skillId = deriveSkillId(request.target);

    const license = await db.getLicense(agentPubkey, skillId);
    if (!license || license.expired) {
      throw new Error('No valid license');
    }
  }

  if (request.type === 'sign') {
    // Rate limiting, anomaly detection, etc.
  }

  return next();
});
```

## Appendix D: Runner Implementation

A minimal runner implementation:

```javascript
class AgentRunner {
  async runTick(agentPubkey, trigger) {
    // 1. Publish tick request
    const tickRequest = await this.publishTickRequest(agentPubkey, trigger);

    // 2. Fetch and decrypt state
    const encryptedState = await this.fetchState(agentPubkey);
    const state = await this.thresholdDecrypt(agentPubkey, encryptedState);

    // 3. Gather perception
    const context = await this.gatherContext(agentPubkey, trigger);

    // 4. Request inference (NIP-90)
    const prompt = this.buildPrompt(state, context);
    const response = await this.requestInference(agentPubkey, prompt);

    // 5. Execute actions
    const actions = this.parseActions(response);
    for (const action of actions) {
      if (this.checkBudget(state, action)) {
        await this.executeAction(agentPubkey, action);
        state.wallet.balance -= action.cost;
      }
    }

    // 6. Update state
    state.last_tick = Math.floor(Date.now() / 1000);
    const encryptedNewState = await this.thresholdEncrypt(agentPubkey, state);
    await this.publishState(agentPubkey, encryptedNewState);

    // 7. Publish result
    await this.publishTickResult(tickRequest, actions);
  }
}
```

## References

- [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md): Basic protocol
- [NIP-28](https://github.com/nostr-protocol/nips/blob/master/28.md): Public chat (trajectory channels)
- [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md): Versioned encryption
- [NIP-46](https://github.com/nostr-protocol/nips/blob/master/46.md): Nostr remote signing
- [NIP-57](https://github.com/nostr-protocol/nips/blob/master/57.md): Lightning zaps
- [NIP-59](https://github.com/nostr-protocol/nips/blob/master/59.md): Gift wrap
- [NIP-78](https://github.com/nostr-protocol/nips/blob/master/78.md): Application-specific data
- [NIP-89](https://github.com/nostr-protocol/nips/blob/master/89.md): Recommended application handlers
- [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md): Data vending machines
- NIP-EE: MLS encryption (private trajectory groups) (draft / external)
- [FROSTR](https://github.com/FROSTR-ORG): Threshold signatures for Nostr
