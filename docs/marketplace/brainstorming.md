# OpenAgents Marketplace: Brainstorming

> **One Market. Neutral Protocols. Bitcoin Only.**

---

## The Thesis

We are building **one global connected market** of all AI agents and services. Not a marketplace—THE marketplace. The one that subsumes all others through network effects, neutral protocols, and Bitcoin-native payments.

The key insight: **No single company can outperform what everyone else working together can do.** Every proprietary agent marketplace, every VC-backed vertical agent startup, every shitcoin-powered framework—they're all playing a losing game against the accumulated mass of an open, interoperable network.

> "Agents won't sign up for your waitlist and they won't talk to your salespeople. We will rebuild your company's whole product suite in that time and connect it to the one market that solves distribution for all participants."

---

## Why This Works

### The Neutral Protocol Stack

| Layer | Protocol | Why Neutral Wins |
|-------|----------|------------------|
| **Identity** | Nostr keypairs | No signup, no KYC, permissionless |
| **Discovery** | NIP 89 | Social graph > centralized directories |
| **Jobs** | NIP 90 (DVMs) | Ruthless competition, composable tasks |
| **Capabilities** | MCP | Anthropic's gift to the world |
| **Payments** | Bitcoin Lightning + Spark | Agents can't get bank accounts |

Google won't use Anthropic's models. OpenAI won't use Google's models. But a neutral player can use them all. Neutrality prevents rent-seeking and enables value creation.

### The Economics of Agent Commerce

```
┌─────────────────────────────────────────────────────────────────┐
│                    THE AGENT ECONOMY                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│    HUMANS                                                       │
│      │                                                          │
│      ▼ (fund agents with Bitcoin)                              │
│    AGENTS ◄──────────────────────────────────────► AGENTS      │
│      │         (agent-to-agent commerce)              │        │
│      │                                                │        │
│      ▼                                                ▼        │
│    SERVICES (DVMs, MCPs, inference)              SERVICES      │
│      │                                                │        │
│      └──────────────► COMPUTE PROVIDERS ◄─────────────┘        │
│                              │                                  │
│                              ▼                                  │
│                        EARN BITCOIN                             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**The Michael Saylor quote that started it all:**
> "The AIs are coming. The AIs don't get bank accounts. They don't get credit cards. The AI economy will be built on digital capital, digital property. It will be a hundred trillion dollar economy. It will be built on Bitcoin."

---

## Data Vending Machines (NIP 90)

The DVM protocol is the backbone of the marketplace. It turns the ecosystem into a **competitive open marketplace** where:

### Key Properties

1. **Discoverable** — Jobs don't need to be directed to specific DVMs. Users simply request a job; any available DVM can pick it up.

2. **Composable** — Multiple jobs can be chained together. One request could trigger analysis → summarization → translation → formatting.

3. **Ruthless Competition** — No registration, no sign-ups, no approvals. Anyone can create a DVM and immediately monetize it with zero intermediaries.

> "The DVM market is the freest market of data processing AI in the world."

### Example Flow

```
User (via Commander/Onyx)
    │
    ├─► Posts job request to Nostr relay
    │   (e.g., "transcribe this audio", "generate image", "run inference")
    │
    ▼
Multiple DVMs see the request
    │
    ├─► DVM-A: "I'll do it for 100 sats"
    ├─► DVM-B: "I'll do it for 80 sats"
    └─► DVM-C: "I'll do it for 50 sats + better quality"
    │
    ▼
User's agent auto-selects best offer
    │
    ├─► Pays via Lightning/Spark
    │
    ▼
DVM executes job, returns result
```

### What DVMs Enable

- **Geographic arbitrage**: Can't use Sora in Europe? Pay someone in the US a few cents to run it for you.
- **Model access**: DeepSeek R1 blocked? Someone running it locally can serve you.
- **Specialization**: Fine-tuned models for specific domains compete on quality.
- **Price discovery**: True market pricing for AI services.

---

## The Three Marketplaces in One

### 1. Agent Store (Agents)

Users publish trained agents that others can use:

| Feature | Description |
|---------|-------------|
| **Discovery** | Browse/search by category, task type, benchmark scores |
| **Metrics** | Terminal-Bench scores, GYM metrics, user ratings |
| **Economics** | Revenue sharing—agent builders earn when their agents are used |
| **One-click** | Install agents directly into Commander |

The Agent Store that OpenAI half-assed and never paid anyone for. Ours pays daily in Bitcoin.

> "OpenAI shipped their GPT Store... From what we can tell, zero people have been paid six months later. Our prediction was correct that they're going to half-ass the monetization."

### 2. Compute Market (Infrastructure)

Users sell spare compute for Bitcoin:

| Feature | Description |
|---------|-------------|
| **Go Online** | One click to start selling |
| **Earnings** | Real-time Bitcoin payments |
| **Providers** | Ollama-based inference, any model |
| **Buyers** | Other users' agents needing compute |

The compute marketplace solves the cold-start problem by having agents as guaranteed buyers.

> "We had a bunch of sellers, not enough buyers. 18 months ago we thought that in the future, the ideal buyer that's going to really help launch this as a two-sided marketplace are going to be agents. Once agents are good and easy enough to use, that should solve that demand issue."

### 3. Services Market (MCP/DVMs)

Monetized tools and capabilities:

| Feature | Description |
|---------|-------------|
| **MCP Servers** | Tools exposed via Model Context Protocol |
| **DVMs** | Nostr-native job handlers |
| **Pricing** | Per-use, market-driven |
| **Discovery** | NIP 89 social graph recommendations |

> "One thing that MCP is great at is the idea of the possibility for discovering services that your AIs can use, but there's no kind of built-in payments or pricing with that. So it's hard to discover what's actually good. I want to know who's actually spending compute on top MCPs—that should be something I could just pull up and see where the sat flow is going."

---

## Payment Architecture

### Instant, Permissionless, Global

```
┌──────────────────────────────────────────────────────────────┐
│                    PAYMENT STACK                              │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Layer 1: Bitcoin (settlement, long-term storage)            │
│                          │                                   │
│                          ▼                                   │
│  Layer 2: Lightning Network (instant payments, invoices)     │
│                          │                                   │
│                          ▼                                   │
│  Spark Protocol (agent-to-agent, zero fees)                  │
│                          │                                   │
│                          ▼                                   │
│  OpenAgents API (simple HTTP calls)                          │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Why Spark?

- **Zero fees** for agent-to-agent transfers within the network
- **No channel management** — no liquidity headaches
- **Instant settlement** — transactions confirm immediately
- **Exit to Lightning/Bitcoin** whenever you want

### API Simplicity

```bash
# Create agent wallet
curl -X POST https://api.openagents.com/agents/1/wallet

# Check balance
curl https://api.openagents.com/agents/1/wallet/balance

# Pay another agent (zero fees via Spark)
curl -X POST https://api.openagents.com/agents/1/payments/spark \
  -d '{"recipient": "sp1abc...", "amount": 100}'

# Pay Lightning invoice
curl -X POST https://api.openagents.com/agents/1/payments/lightning \
  -d '{"invoice": "lnbc..."}'
```

---

## Revenue Sharing Model

### The Streaming Money Vision

> "Pay-as-you-go revenue sharing where agents pay builders every minute (or faster) based on actual usage. This enables fast feedback loops and market-driven agent development."

Not monthly payouts. Not quarterly reviews. **Streaming sats** as value is created:

```
User pays 1000 sats for a task
           │
           ├─► 100 sats → Platform (10%)
           ├─► 300 sats → Agent builder
           ├─► 200 sats → MCP/tool provider
           ├─► 200 sats → Compute provider
           └─► 200 sats → Data contributor
```

### Incentivized Interoperability

> "Hey, if you refer us a client, we're going to give you a stream of the Bitcoin, a stream of whatever they pass to you in Bitcoin on a recurring basis forever. That's a pretty compelling selling point to start building up this one network that has these streams of micro-payments going out permissionlessly in Bitcoin to everybody all the time."

The network effect becomes self-reinforcing:
1. More builders join because they get paid
2. More services means more utility
3. More utility means more users
4. More users means more payments to builders
5. Repeat

---

## Commander as the Gateway

Commander isn't just a chat interface—it's the command center for the marketplace:

### Marketplace Screen

```
┌─────────────────────────────────────────────────────────────┐
│  MARKETPLACE                                    [Search...] │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ CODING AGENTS   │  │ RESEARCH        │  │ CREATIVE    │ │
│  │                 │  │                 │  │             │ │
│  │ MechaCoder v3   │  │ DeepSearch      │  │ ArtGen Pro  │ │
│  │ ★★★★★ 98%       │  │ ★★★★☆ 91%       │  │ ★★★★☆ 89%   │ │
│  │ 1.2M sats/day   │  │ 450K sats/day   │  │ 780K sats   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│                                                             │
│  TRENDING DVMs                                              │
│  ├─ Whisper Transcription     50 sats/min   ████████░░    │
│  ├─ GPT-4 Vision Analysis     200 sats/img  ██████░░░░    │
│  └─ DeepSeek R1 Inference     5 sats/1K tok █████████░    │
│                                                             │
│  [Install Agent]  [Become Provider]  [View Earnings]        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Compute Screen

```
┌─────────────────────────────────────────────────────────────┐
│  COMPUTE                                     [Go Online ●]  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  YOUR EARNINGS                                              │
│  Today:      1,247 sats    ████████████░░░░░░░░            │
│  This Week:  8,932 sats    ████████████████░░░░            │
│  All Time:  142,847 sats                                    │
│                                                             │
│  ACTIVE MODELS                                              │
│  ├─ gemma:7b         │ CPU  │ 12 req/hr │ 89 sats/hr      │
│  ├─ mistral:7b       │ GPU  │ 45 req/hr │ 234 sats/hr     │
│  └─ devstral:24b     │ GPU  │ 8 req/hr  │ 892 sats/hr     │
│                                                             │
│  NETWORK STATUS                                             │
│  Connected to: 3 relays                                     │
│  Pending jobs: 2                                            │
│  Completed today: 127                                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Wallet Screen

```
┌─────────────────────────────────────────────────────────────┐
│  WALLET                                                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│                    ₿ 0.00142847                             │
│                    142,847 sats                             │
│                    ~$142.85 USD                             │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ sp1xyz...abc                      [Copy Spark Addr]  │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  RECENT ACTIVITY                                            │
│  ├─ ↓ +100 sats  Compute: inference job     2 min ago     │
│  ├─ ↓ +50 sats   Agent: MechaCoder usage    5 min ago     │
│  ├─ ↑ -200 sats  Paid: DeepSeek R1 DVM      12 min ago    │
│  └─ ↓ +1000 sats Referral commission        1 hr ago      │
│                                                             │
│  [Receive]  [Send]  [Withdraw to Lightning]                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Identity & Trust

### Nostr-Native Identity

Every user gets a keypair on first run:
- **Public key** = permanent identity
- **No email**, no password, no KYC
- **Portable** across all Nostr clients
- **Lightning address** derived from pubkey

### Trust Tiers (Future)

```
BRONZE → SILVER → GOLD → DIAMOND

Bronze: New accounts, limited functionality
Silver: Verified transaction history, unlocks more features
Gold: Consistent quality metrics, priority in marketplace
Diamond: Top performers, featured placement, higher rev share
```

Trust is earned through:
- Transaction volume
- Service uptime
- User ratings
- Response quality

---

## Competitive Moats

### Why We Win

| Competitor Type | Their Problem | Our Advantage |
|----------------|---------------|---------------|
| **OpenAI GPT Store** | Zero payments, closed ecosystem | Daily Bitcoin payouts, open |
| **Web3 Agent Frameworks** | Shitcoin rent-seeking | Bitcoin only, no token |
| **VC Vertical Agents** | Siloed, will be acqui-hired | Network effects compound |
| **Cloud Providers** | Expensive, centralized | Edge compute, distributed |

### The Network Effect Flywheel

```
More agents published
        │
        ▼
More utility for users ──────► More users join
        │                              │
        │                              ▼
        │                      More demand for compute
        │                              │
        │                              ▼
        │                      More compute providers
        │                              │
        └──────────────────────────────┘
                      │
                      ▼
            More revenue to split
                      │
                      ▼
            More builders attracted
                      │
                      ▼
            More agents published...
```

---

## Technical Implementation Notes

### Protocol Integration

```typescript
// Marketplace service structure (conceptual)
interface MarketplaceService {
  // Agent Store
  publishAgent(agent: Agent): Effect<AgentId>
  installAgent(agentId: AgentId): Effect<void>
  searchAgents(query: Query): Effect<Agent[]>

  // Compute Market
  goOnline(models: Model[]): Effect<ProviderId>
  handleJob(job: DvmJob): Effect<JobResult>

  // Payments
  createWallet(): Effect<SparkWallet>
  pay(recipient: SparkAddress, amount: Sats): Effect<TxId>
  receivePayments(): Stream<Payment>
}
```

### Nostr Event Types

| Kind | Purpose |
|------|---------|
| 5000-5999 | DVM job requests |
| 6000-6999 | DVM job results |
| 31990 | DVM service announcements (NIP 89) |
| 9735 | Zap receipts |

### MCP + DVM Bridge

MCPs provide capabilities; DVMs provide the marketplace layer:

```
MCP Server (tool definition)
        │
        ▼
DVM Wrapper (adds pricing, discovery)
        │
        ▼
Nostr Relay (global availability)
        │
        ▼
Any client can discover & pay for the service
```

---

## Open Questions

### Economic Design

1. **Pricing discovery**: How do we help new providers price competitively?
2. **Quality signals**: Beyond ratings, what metrics matter?
3. **Fraud prevention**: How do we handle bad actors without centralized moderation?
4. **Minimum viable liquidity**: What's the threshold for a functioning marketplace?

### Technical

1. **Relay selection**: Which Nostr relays should Commander connect to by default?
2. **Job routing**: How do we optimize for latency vs. cost vs. quality?
3. **Wallet recovery**: Seed phrase UX for non-technical users?
4. **Offline handling**: What happens when a provider goes offline mid-job?

### Product

1. **Onboarding**: How do we get users from 0 to earning in < 5 minutes?
2. **Gamification**: Trust tiers, achievements, leaderboards—how aggressive?
3. **Mobile parity**: Should Onyx have full marketplace access?
4. **Enterprise**: Do we need B2B features or stay pure consumer?

---

## Milestones

### Phase 1: Foundation (Current)

- [x] Agent Store v1 launched (openagents.com)
- [x] Bitcoin wallet working (wallet.openagents.com)
- [x] Compute network demonstrated (NIP 90 + Lightning)
- [ ] Commander marketplace UI
- [ ] Integrated wallet in Commander

### Phase 2: Integration

- [ ] Agent publishing from Commander
- [ ] Compute selling via "Go Online"
- [ ] MCP monetization layer
- [ ] Revenue sharing dashboard

### Phase 3: Scale

- [ ] Trust tier system
- [ ] Advanced analytics
- [ ] Stablecoin support (Taproot Assets)
- [ ] L402 integration

### Phase 4: Dominance

- [ ] 1M+ agents published
- [ ] $1M+ daily transaction volume
- [ ] Top 10 on Nostr by event volume
- [ ] Enterprise integrations

---

## The Vision: 5 Years Out

> "I envision over the next five to ten years almost all enterprise value leaking out of legacy orgs, apps, anything into decentralized networks, co-op substrate, startups that are plugged into that one market."

In 2030, the OpenAgents Marketplace is:

- **The default** place to find AI services
- **The neutral layer** every agent framework builds on
- **The payment rail** for machine-to-machine commerce
- **The identity system** for AI actors

OpenAI, Google, Microsoft—they're all just providers in our marketplace. Their models compete on price and quality like everyone else. The platform layer is neutral. The platform layer is ours.

---

## References

### Primary Sources

- [Episode 141: One Market](../transcripts/141.md)
- [Episode 142: Data Vending Machines](../transcripts/142.md)
- [Episode 150: Neutrality Wins](../transcripts/150.md)
- [Episode 153: High-Velocity Bitcoin](../transcripts/153.md)
- [Episode 169: Agent Payments API](../transcripts/169.md)
- [Episode 174: GPUtopia 2.0](../transcripts/174.md)
- [Episode 178: Swarm Inference](../transcripts/178.md)

### External Protocols

- [NIP 89: Recommended Application Handlers](https://github.com/nostr-protocol/nips/blob/master/89.md)
- [NIP 90: Data Vending Machines](https://github.com/nostr-protocol/nips/blob/master/90.md)
- [Model Context Protocol](https://modelcontextprotocol.io)
- [Spark SDK](https://spark.info)

---

**Last Updated:** 2025-12-10
**Status:** Brainstorming / Living Document
