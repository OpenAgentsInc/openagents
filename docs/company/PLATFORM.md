# The OpenAgents Platform

> "Connect a repo and a credit card. Go do something else. We'll take it from here."

## The Vision

Software engineering is about to undergo its biggest transformation since the compiler. We're building the platform where you can:

1. **Connect** your GitHub repository
2. **Add credits** via Stripe or Bitcoin Lightning
3. **Give instructions** like "Implement the user authentication system" or "Fix all failing tests"
4. **Walk away** — the agent works autonomously while you sleep, think, or ship elsewhere

When you return, you find pull requests ready for review, each with a complete audit trail of reasoning, decisions, and test results. No babysitting. No prompt engineering. Just results.

This isn't a copilot. It's an **autopilot**.

---

## The Core Loop

### 1. The "Sanity Check" — Trust in 60 Seconds

The first 60 seconds are trust-building. When you connect a repo:

- Agent immediately starts analyzing, showing visible progress
- You see a streaming "sci-fi HUD" of the agent traversing your codebase
- Tool use is visible — file reads, searches, navigations happen in real-time
- You can **nudge** if instructions weren't clear, but you don't have to

This isn't a demo. It's the agent proving competence on YOUR code. Once you trust it, you can walk away.

### 2. Autopilot Dashboard — Command Your Fleet

Not one agent. A **fleet**:

- Manage multiple agents simultaneously in a cockpit view
- Hotkeys like StarCraft — press `A` to spawn a new agent
- Create agent groups and swap between them with hotkeys
- See a visual canvas with positions, status, and payment flows
- Real-time multiplayer sync across team members

The UI is a futuristic command center, not a chat window.

### 3. Overnight Agents — Wake Up to PRs

Schedule work before you sleep:

- Queue multiple feature requests
- Set budget caps, file scope restrictions, no-destructive-ops mode
- Agent runs reflection loops every 15 minutes to stay on track
- Full audit trail of every thought and action
- Morning notification with summary: PRs created, tests passed, issues found

You go to bed with a backlog. You wake up to a stack of reviewable PRs.

---

## What We're Building

### The Web Platform (openagents.com)

**Stack:** Rust/Actix backend + WASM frontend

| Epic | Capability |
|------|------------|
| **Web Activation** | Landing → GitHub OAuth → Repo select → Analysis in < 60 seconds |
| **Auth & Identity** | Unified identity across web and desktop, transparent trust controls |
| **GitHub Integration** | PR workflow with diff preview, incremental indexing, safe writes |
| **Web Chat & Session** | Streaming chat with tool visibility, session persistence |
| **Billing** | Credit-based with Stripe, pay-as-you-go visualization |

### The Desktop App (openagents)

**Stack:** Rust + wgpui (native webview shell)

| Epic | Capability |
|------|------------|
| **Desktop Sync** | QR/code pairing, shared identity and billing with web |
| **Autopilot Dashboard** | Multi-agent cockpit with hotkeys and visual canvas |
| **Autonomous Agents** | Overnight work, checkpointing, audit trails |
| **Local Inference** | On-device models when cloud isn't needed or allowed |
| **Swarm Compute** | Sell spare GPU cycles for Bitcoin |

---

## The Economic Model

### Credit-Based Billing

We don't do subscriptions for capacity you don't use. You buy credits, you use credits:

| Plan | Monthly | Credits | Notes |
|------|---------|---------|-------|
| **Free** | $0 | 10,000 (signup) | Get started, prove value |
| **Pro** | $20 | 500,000 | Full features, all models |
| **Team** | $15/seat | 300,000/seat | Shared billing, team features |
| **Enterprise** | Custom | Custom | SSO, compliance, SLA |

**Key features:**
- Real-time balance display with burndown during sessions
- Cost "preflight" for expensive operations ("this may cost ~$X; continue?")
- Hard budget caps per day/week/month with auto-stop
- Session-level receipts ("this thread cost $X")

### Streaming Money

This is the future of value-aligned payments:

- **Creators receive micropayments in real-time** as their skills are used
- Revenue sharing pays out every minute, not monthly
- Live cost streaming during sessions — pay-as-you-go visualization
- Bitcoin Lightning for instant, low-friction settlements

When you use someone's skill, they get paid immediately. Not in 30 days. Not after payment processing. **Now**.

---

## The Skills Marketplace

> "Don't build agents, build skills." — Anthropic

Skills are reusable expertise that make agents better. The marketplace lets you:

### As a User
- **Install skills one-click** and have them immediately available to agents
- **Day 30 > Day 1**: Your Claude improves over time, learning from patterns
- Skills compose with MCP servers (MCP = connectivity, Skills = expertise)
- Share skills with your team instantly

### As a Creator
- Publish skills with schema, pricing, versioning
- Real-time micropayments as users call your skills
- Transparent fee breakdown and earnings dashboard
- Scripts as tools (Python/Node/Bash with sandboxed execution)

### Revenue Sharing

| Share | Recipient |
|-------|-----------|
| 50-60% | Skill creator |
| 20-30% | Platform |
| 10-20% | Optional: referrer, contributor, cause |

The platform takes a cut. But creators get the majority. Always.

---

## The Swarm Compute Network

> "Sell spare compute for Bitcoin by clicking 'Go Online'."

Your laptop, your GPU, your spare cycles — they're worth money:

### As a Provider
1. Click "Go Online" in the app
2. Set your pricing (per 1k tokens, per minute)
3. Hardware capability auto-detected
4. Jobs come in, you get paid in Lightning

### As a Consumer
1. Need more compute than local hardware provides?
2. See available providers, their pricing, latency estimates
3. Submit job, stream results back
4. Pay instantly via Lightning

### Provider Dashboard
```
┌─────────────────────────────────────────────────────────────┐
│                    Provider Dashboard                        │
├─────────────────────────────────────────────────────────────┤
│  Status: 🟢 Online                                          │
│                                                              │
│  Today's Earnings: ₿ 0.00023 (~$15.30)                      │
│  Jobs Completed: 47                                          │
│  Tokens Generated: 1,234,567                                 │
│                                                              │
│  ┌─────────────────────────────────────┐                    │
│  │ ████████████░░░░░░░░░░  48% GPU     │                    │
│  │ ██████░░░░░░░░░░░░░░░░  25% RAM     │                    │
│  └─────────────────────────────────────┘                    │
│                                                              │
│  [Withdraw to Wallet]  [Adjust Pricing]  [Go Offline]       │
└─────────────────────────────────────────────────────────────┘
```

This is not a blockchain play. It's a **protocol play**:
- No tokens, no speculation
- Just Bitcoin/Lightning for settlement
- Nostr (NIP 89/90) for discovery
- Real work, real payment

---

## Open Protocols — One Market

We're building on open protocols, not walled gardens:

| Protocol | Purpose |
|----------|---------|
| **Nostr (NIP 89/90)** | Agent discovery, job distribution, reputation |
| **Bitcoin Lightning** | Instant micropayments, cross-border settlements |
| **MCP** | Tool connectivity, capability discovery |

### Why Open?

1. **Network effects compound**: 2^N value from Reed's Law, not limited by our engineering capacity
2. **Competition is healthy**: We say "Team Actually Open AI" for a reason
3. **Interoperability wins**: Your agents can work with anyone's tools, anywhere

The goal is **one global market for AI agents** — not dozens of fragmented walled gardens.

---

## Enterprise & Procurement

We know what enterprise buyers need:

### Security Packet (Ready to Download)
- Data flow diagram
- Subprocessors list
- Retention policies
- Encryption details
- Incident response contact
- DPA template
- SOC2 status

### Trust Controls
- Provider/model allow-lists
- "No external network" policy mode
- Regional data residency (US-only initially)
- Tamper-evident audit logs with hash chain
- SSO/SCIM provisioning

### Procurement Enablers
- Invoice terms (net-30, PO)
- Pilot programs
- Centralized org billing
- Verifiable account deletion with certificates

---

## Metrics That Matter

| Metric | Target |
|--------|--------|
| **Time to Sanity Check** | < 60 seconds |
| **OAuth Completion Rate** | > 70% |
| **Trust Established Rate** | > 80% |
| **Go AFK Rate** | > 50% |
| **Overnight Success Rate** | > 90% |
| **Conversion to Paid** | > 10% |
| **Skills Published** | 100+ in Q1 |

---

## The 21 Epics

Our roadmap is organized into 21 epics, 259 user stories, targeting $100M revenue:

### Critical Path (P0)
1. **EPIC-00**: Web Activation & Sanity Check
2. **EPIC-01**: Auth & Identity
3. **EPIC-02**: GitHub Integration & PR Loop
4. **EPIC-03**: Web Chat & Session
5. **EPIC-12**: Billing & Streaming Money
6. **EPIC-06**: ChatService Layer
7. **EPIC-07**: Permissions, Secrets & Safety

### Core Engine (P1)
8. **EPIC-04**: Desktop Download & Sync
9. **EPIC-05**: Agents, Models & Local Inference
10. **EPIC-08**: Storage & Persistence
11. **EPIC-09**: UI Runtime & Rendering
12. **EPIC-10**: Surfaces
13. **EPIC-11**: Tools & Build/Test Loop
14. **EPIC-17**: Autopilot & Multi-Agent
15. **EPIC-18**: Autonomous & Overnight Agents
16. **EPIC-15**: Observability, APM & Ops

### Ecosystem (P2)
17. **EPIC-13**: Marketplace & Skills
18. **EPIC-14**: Reliability & Offline
19. **EPIC-16**: Enterprise & Procurement
20. **EPIC-19**: Open Protocols & One Market
21. **EPIC-20**: Swarm Compute Network
22. **EPIC-21**: Benchmarks & Evals

---

## Philosophy

### Worse is Better
We ship what works. Simplicity over completeness. Correct enough beats theoretically perfect. Consistency bends for simplicity. We iterate in public.

### Copilots are Training Wheels
4.5 actions per minute with a human at the keyboard.
19 actions per minute with Autopilot.
**4x productivity gain**. That's not assistance — that's automation.

### AI Operator → AI Investor
Today: You spend time operating AI tools.
Tomorrow: You invest attention in AI agents.
The paradigm shift is from "using" to "deploying."

### Open Beats Closed
DeepMind announced an "agent protocol."
We're building on Nostr + Lightning + MCP.
They're making an intranet. We're making the internet.

---

## The Bottom Line

OpenAgents isn't just building an AI coding tool. We're building:

1. **A platform** where autonomous agents do real work while you sleep
2. **A marketplace** where skill creators earn real money in real-time
3. **A compute network** where anyone can sell spare cycles for Bitcoin
4. **An open protocol ecosystem** that compounds network effects globally

Connect a repo. Add credits. Walk away.

We'll take it from here.
