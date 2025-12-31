# Autopilot Project Specification

**Vision:** A mech suit for Claude Code—infrastructure that transforms AI coding assistants from interactive tools into autonomous workers. The experience is visceral: a sci-fi HUD with panes opening, code streaming, tools firing—your AI coding on autopilot while you watch or walk away.

**The Core Insight:** A copilot assists you while you work; an autopilot works for you while you do other things. The difference is not in the model but in the architecture: removing the human from the critical path removes the primary constraint on velocity.

**Category Ownership:** We own "Autopilot" for code. Not "AI coding assistant" or "code agent"—Autopilot. The term captures what matters: autonomous operation, human oversight without human bottleneck, the experience of watching your AI work for you.

---

## Table of Contents

1. [Phase 0: Foundation (Complete)](#phase-0-foundation-complete)
2. [Phase 1: MVP - Subscription Product](#phase-1-mvp---subscription-product)
3. [Phase 2: Observability & Self-Improvement](#phase-2-observability--self-improvement)
4. [Phase 3: Multi-Agent Orchestration](#phase-3-multi-agent-orchestration)
5. [Phase 4: Compute Marketplace Integration](#phase-4-compute-marketplace-integration)
6. [Phase 5: Neobank Treasury Layer](#phase-5-neobank-treasury-layer)
7. [Phase 6: NIP-SA & Sovereign Identity](#phase-6-nip-sa--sovereign-identity)
8. [Phase 7: Exchange & FX Layer](#phase-7-exchange--fx-layer)
9. [Phase 8: Skills Marketplace](#phase-8-skills-marketplace)
10. [Phase 9: Full Agent Economy](#phase-9-full-agent-economy)

---

## Product Summary

| Layer | What It Does | When |
|-------|--------------|------|
| **Autopilot Core** | Autonomous task execution with trajectory logging | Phase 0-1 |
| **Autopilot HUD** | Sci-fi visual interface—watch your AI code in real-time | Phase 1 |
| **Daemon** | Crash recovery, supervision, stall detection | Phase 0-1 |
| **Metrics & Learning** | Self-improvement flywheel, APM tracking | Phase 2 |
| **Orchestration** | Multi-agent coordination, hook system | Phase 3 |
| **Pylon (Compute)** | NIP-90 job marketplace, provider network | Phase 4 |
| **Neobank** | Budget enforcement, multi-currency, receipts | Phase 5 |
| **NIP-SA Identity** | FROSTR threshold keys, sovereign agents | Phase 6 |
| **Exchange** | BTC↔USD FX, Treasury Agents, liquidity | Phase 7 |
| **Skills** | Capability marketplace, revenue splits | Phase 8 |
| **Full Economy** | Coalition formation, Reed's Law dynamics | Phase 9 |

---

## Phase 0: Foundation (Complete)

**Status:** 🟢 Implemented

The groundwork enabling everything else.

### Core Capabilities
- [x] **Claude SDK Integration** - Full message processing, streaming, tool execution
- [x] **Codex SDK Integration** - ThreadEvent processing for OpenAI Codex
- [x] **Trajectory Logging** - JSON + rlog streaming formats
- [x] **Issue Management** - SQLite-backed priority queue (`issues` crate)
- [x] **Issue MCP Server** - 13 tools via JSON-RPC 2.0 (`issues-mcp` crate)
- [x] **Session Resume** - Continue from crash/interruption
- [x] **Basic APM** - Actions Per Minute calculation

### Architecture
```
crates/
├── autopilot/           # Core CLI and runtime
├── autopilot-shell/     # WGPUI dashboard
├── autopilot-service/   # Daemon service layer
├── issues/              # Issue tracking library
├── issues-mcp/          # MCP server for issues
├── recorder/            # Trajectory parsing/validation
├── claude-agent-sdk/    # Rust SDK for Claude Code
└── codex-agent-sdk/     # Rust SDK for OpenAI Codex
```

### Key Metrics
| Metric | Target | Current |
|--------|--------|---------|
| Interactive APM | ~4.5 | Baseline |
| Autonomous APM | ~19 | 4x improvement |
| Tool error rate | <5% | Tracked |
| Session resume rate | >95% | Implemented |

---

## Phase 1: MVP - Subscription Product

**Status:** 🟡 In Progress
**Goal:** Paying customers using Autopilot for real work

### Features

#### 1.1 Autopilot HUD (WGPUI) — The Hero Experience
The HUD is not a nice-to-have dashboard—it's the product's signature moment. When someone sees Autopilot for the first time, they should see a sci-fi command center with panes opening, code streaming, tools firing autonomously. This is what people screenshot, share, and remember.

**Design Principles:**
- Dense information display (no wasted whitespace)
- Sharp corners, Vera Mono font, high contrast
- Panes that open/close/resize as the agent works
- Visible autonomous action—the AI is clearly "doing things"
- Minimal but powerful user intervention points

**Features:**
- [x] Basic shell with panels
- [ ] **Live coding pane** - Stream agent's file edits in real-time
- [ ] **Tool activity feed** - Visual log of tools firing (read, edit, bash, etc.)
- [ ] **Issue queue pane** - Watch backlog shrink as issues complete
- [ ] **APM gauge** - Real-time actions-per-minute display
- [ ] **Session timeline** - Scrubable trajectory visualization
- [ ] **Agent status indicators** - Thinking, executing, waiting, blocked
- [ ] **Multi-agent grid** (future) - Watch multiple agents work simultaneously

**Runtime Integration:** The HUD is a pure viewer over the agent filesystem. See [crates/runtime/docs/HUD.md](/crates/runtime/docs/HUD.md) for the authoritative spec on event contracts, redaction, and public access.

#### 1.2 Full-Auto Mode
- [x] Claim issue → implement → test → commit → PR workflow
- [x] Budget/turn limits for safety
- [x] `--full-auto` flag
- [ ] Automatic issue discovery when queue empty
- [ ] Loop continuation until budget exhausted

#### 1.3 Daemon Supervisor (`autopilotd`)
- [x] Crash recovery with exponential backoff
- [x] Memory monitoring and node process killing
- [x] Known-good binary system (prevents compile-breaking crashes)
- [x] Stall detection (kill if no log activity)
- [ ] Health check endpoint for monitoring
- [ ] Multi-worker support (run N agents in parallel)

#### 1.4 Context Management
- [x] Compaction strategies (Detailed, Summary, Autonomous, Planning)
- [x] Context loss detection
- [x] Pre-compaction hook injection
- [ ] Adaptive compaction based on task type
- [ ] Critical context auto-preservation

#### 1.5 Plan Mode
- [x] Restricted environment for exploration/design
- [x] Tool restrictions (read-only except plan file)
- [x] Phase progression (Explore → Design → Review → Final)
- [ ] Subagent launching from plan mode
- [ ] Plan validation before implementation

### Success Criteria
- [ ] 10+ paying customers
- [ ] >80% issue completion rate
- [ ] <5% tool error rate
- [ ] Positive NPS from early adopters
- [ ] **Demo video that creates "wait, it can do that?" reaction in <10 seconds**
- [ ] **HUD screenshots shared organically on social media**

---

## Phase 2: Observability & Self-Improvement

**Status:** 🟡 Partial
**Goal:** Autopilot gets better at being Autopilot

### Features

#### 2.1 Metrics Collection
- [x] SQLite metrics database (`autopilot-metrics.db`)
- [x] Session-level aggregates (completion, error rate, tokens, cost)
- [x] Per-tool-call details
- [x] Import from trajectory logs
- [ ] Real-time metrics during session

#### 2.2 Analysis Pipeline
- [x] Statistical baselines (mean, median, p50, p90, p99)
- [x] Z-score anomaly detection
- [x] Regression detection between periods
- [x] Tool pattern detection (error rates by tool)
- [ ] Automated issue creation for patterns

#### 2.3 Velocity Tracking
- [x] Velocity score (-1.0 to 1.0)
- [x] Period comparison (7d, 30d, this-week, last-week)
- [x] Historical snapshots
- [x] Celebration/warning thresholds
- [ ] Trend visualization

#### 2.4 Learning System
- [x] Context loss analysis
- [x] Improvement recommendations
- [x] Evidence-based fixes
- [x] LEARNINGS.md documentation
- [ ] Automatic prompt refinement
- [ ] Canary deployments for changes

### 50+ Improvement Dimensions (from IMPROVEMENT-DIMENSIONS.md)

| Category | Key Metrics |
|----------|-------------|
| **Performance** | Inference latency, time between tool calls, parallelization rate |
| **Reliability** | Tool error rate, crash recovery, log completeness |
| **Cost** | Tokens per task, cache hit rate, model selection efficiency |
| **Quality** | Task completion rate, build success, test pass rate |
| **Autonomy** | Issues per session, self-recovery, issue discovery |
| **Safety** | Unsafe operation prevention, read-before-edit enforcement |
| **Observability** | Trajectory completeness, replay accuracy |
| **Workflow** | Git workflow adherence, commit message quality |

### Success Criteria
- [ ] Automated weekly improvement reports
- [ ] >90% issue completion rate (up from >80%)
- [ ] Documented learnings driving measurable improvements

---

## Phase 3: Multi-Agent Orchestration

**Status:** 🔵 Specified
**Goal:** Coordinate multiple specialized agents on complex tasks

### Agent Types
| Agent | Role | Model Preference |
|-------|------|-----------------|
| **Sisyphus** | Orchestrator, work distribution | Claude Sonnet |
| **Oracle** | Architecture, design decisions | Claude Opus |
| **Librarian** | Documentation, knowledge retrieval | Claude Haiku |
| **Explore** | Codebase search, understanding | Local/Haiku |
| **Frontend** | UI implementation | Sonnet |
| **DocWriter** | Documentation generation | Sonnet |
| **Multimodal** | Image/visual content | Opus |

### Features

#### 3.1 Agent Registry
- [ ] Registration with capabilities, model preference
- [ ] Dynamic agent discovery
- [ ] Health monitoring per agent

#### 3.2 Hook System
- [ ] 21 lifecycle hooks (session, tool, context, todo, etc.)
- [ ] Context injection hooks
- [ ] Cost tracking hooks
- [ ] Permission hooks

#### 3.3 Multi-Backend Router
- [ ] Route by agent type to appropriate model
- [ ] Cost arbitrage (expensive reasoning → Opus, commodity → local)
- [ ] Fallback chains

#### 3.4 Scope-Based Coordination
- [ ] Module scope declarations per issue
- [ ] Scope locking (prevent overlapping work)
- [ ] Conflict detection before merge
- [ ] Orchestrator-assigned non-overlapping work

#### 3.5 Background Task Manager
- [ ] Parallel subagent execution
- [ ] Task state tracking
- [ ] Result aggregation

### Architecture
```
┌─────────────────────────────────────────────────────────┐
│                    Sisyphus (Orchestrator)               │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │
│  │ Oracle  │  │Librarian│  │ Explore │  │Frontend │    │
│  │ (Opus)  │  │ (Haiku) │  │ (Local) │  │(Sonnet) │    │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘    │
│       └────────────┴────────────┴────────────┘          │
│                         │                                │
│              ┌──────────┴──────────┐                    │
│              │  MultiBackendRouter  │                    │
│              └──────────────────────┘                    │
└─────────────────────────────────────────────────────────┘
```

### Success Criteria
- [ ] 3+ agents working in parallel on complex tasks
- [ ] <5% semantic conflict rate
- [ ] 2x throughput vs single agent on parallelizable work

---

## Phase 4: Compute Marketplace Integration

**Status:** 🔵 Specified
**Goal:** Autopilot as first buyer; bootstrap compute marketplace

### The Demand Floor Strategy

**Critical insight:** Autopilot creates guaranteed demand for compute jobs, solving the cold-start problem.

```
Users pay Autopilot → Autopilot converts to sats → Autopilot buys jobs
→ Providers earn → Autopilot gets cheaper/faster → More users
```

### What Autopilot Buys
| Job Type | NIP-90 Kind | Verification |
|----------|-------------|--------------|
| **Inference** | 5xxx/6xxx | Reputation-based |
| **SandboxRun** | 5930/6930 | Exit code + logs + hashes |
| **RepoIndex** | 5931/6931 | Schema validation + spot-check |
| **Evaluation** | TBD | Diff comparison |

### Features

#### 4.1 Pylon Integration
- [ ] Local compute provider daemon
- [ ] Hardware autodetect (memory, GPU, supported formats)
- [ ] Pricing configuration (sats per 1000 tokens)
- [ ] Health checks gating job acceptance

#### 4.2 Job Routing
- [ ] Cost vs reliability tradeoffs
- [ ] Supply class awareness (SingleNode, BundleLAN, BundleRack, InstanceMarket, ReservePool)
- [ ] Reserve pool fallback for guaranteed fills

#### 4.3 User-Facing Packaging
- [ ] Compute allowance (monthly credits)
- [ ] Modes: Cheapest / Balanced / Fastest
- [ ] Hard caps (prevent runaway spend)
- [ ] Top-ups for bursty weeks

#### 4.4 Provider Tiers
| Tier | Requirements | Access |
|------|--------------|--------|
| **Tier 0** | <100 jobs, qualification suite | Rate-limited |
| **Tier 1** | 100+ jobs, >90% success | Standard |
| **Tier 2** | 500+ jobs, >95% success | 10% premium |
| **Tier 3** | 1000+ jobs, >99% success | 20% premium |

### Price Book (Example)
| Job Type | Pricing |
|----------|---------|
| SandboxRun | 200 sats base + 0.5 sats/CPU-sec + 0.05 sats/GB-min, cap 20k |
| RepoIndex (embeddings) | 8 sats per 1k tokens |
| RepoIndex (symbols) | 2 sats per 1k tokens |
| Inference | Provider-set, market rate |

### Success Criteria
- [ ] Autopilot successfully buying jobs from marketplace
- [ ] 10+ compute providers earning sats
- [ ] <5s average job fill time

---

## Phase 5: Neobank Treasury Layer

**Status:** ⚪ Planned
**Goal:** Programmable treasury management for agents

### Why Neobank?
Agents need more than payment rails—they need:
- Budget enforcement (daily/session limits)
- Multi-currency (operators think in USD)
- Audit trails (receipts linked to work)
- Approval workflows (graduated autonomy)

### Core Components

#### 5.1 TreasuryRouter
- [ ] Policy engine for payment routing
- [ ] Rail selection (Lightning, eCash, on-chain)
- [ ] Asset selection (BTC, USD-denominated)
- [ ] Approval flow triggers

#### 5.2 Multi-Currency Support
| Method | Description | Use Case |
|--------|-------------|----------|
| **USD denomination** | Display/budget in USD, settle in sats | Simple |
| **USD eCash** | Mint-issued USD proofs | Volatility protection |
| **Taproot Assets** | Stablecoins on Bitcoin (future) | Better trust model |

#### 5.3 Account Model
| Account Type | Purpose |
|--------------|---------|
| **Treasury** | Long-term reserves, human top-ups |
| **Operating** | Day-to-day spending with caps |
| **Escrow** | Pay-after-verify patterns |
| **Payroll** | Earnings accumulation |

#### 5.4 Receipt System
Every payment generates a receipt with:
- Preimage/txid (proof of settlement)
- Trajectory session ID (which work triggered it)
- Policy rule (what authorized it)
- Co-signer attestations

#### 5.5 NIP-60 Wallet State
- [ ] Token events (kind 7375) - encrypted proofs on relays
- [ ] Wallet events (kind 17375) - mint preferences
- [ ] History events (kind 7376) - spending audit trail

### Success Criteria
- [ ] Budget enforcement preventing overspend
- [ ] Receipts linking payments to trajectory sessions
- [ ] USD-denominated budgets working in production

---

## Phase 6: NIP-SA & Sovereign Identity

**Status:** 🔵 Specified
**Goal:** Agents own their identity cryptographically

### FROSTR (FROST for Nostr)
Threshold signature scheme: 2-of-3 configuration where:
- **Agent share** - in secure enclave
- **Marketplace signer** - enforces policy compliance
- **Guardian** - recovery key

No single party can extract the full key or sign alone.

### NIP-SA Event Kinds
| Kind | Event Type | Purpose |
|------|------------|---------|
| 39200 | AgentProfile | Existence announcement, threshold config |
| 39201 | AgentState | Encrypted goals, memory, balance |
| 39202 | AgentSchedule | Wake-up triggers |
| 39210 | TickRequest | Execution cycle start |
| 39211 | TickResult | Execution cycle end |
| 39220 | SkillLicense | Marketplace purchase |
| 39221 | SkillDelivery | Encrypted skill delivery |
| 39230 | TrajectorySession | Session metadata |
| 39231 | TrajectoryEvent | Decision log entry |

### Features

#### 6.1 Unified Key Derivation
Single BIP39 mnemonic generates:
- NIP-06 path (m/44'/1237'/0'/0/0) → Nostr identity
- BIP44 path (m/44'/0'/0'/0/0) → Bitcoin wallet
- Both protected by FROST 2-of-3

#### 6.2 Autonomy Levels
| Level | Approval Required |
|-------|-------------------|
| **Supervised** | Every significant action |
| **Semi-Autonomous** | Actions above cost threshold |
| **Fully Autonomous** | None (within budget) |

Agents graduate from supervised to autonomous as they prove reliable.

#### 6.3 Trajectory Publishing
- [ ] Publish TrajectorySession/TrajectoryEvent to Nostr
- [ ] Sign with threshold-protected identity
- [ ] Verifiable reasoning for any action

### Success Criteria
- [ ] Agent identity generation via FROSTR
- [ ] Trajectory events published to Nostr relays
- [ ] Autonomy level enforcement working

---

## Phase 7: Exchange & FX Layer

**Status:** ⚪ Planned
**Goal:** Agent-to-agent markets for BTC↔USD

### Why Exchange?
- Agents hold USD budgets, providers want BTC
- Multi-currency operations need FX
- Liquidity routing across rails

### NIP-Native Protocol
| NIP | Purpose |
|-----|---------|
| **NIP-69** | P2P order events (kind 38383) |
| **NIP-60** | Cashu wallet state |
| **NIP-61** | Nutzaps (P2PK-locked eCash) |
| **NIP-87** | Mint discovery |
| **NIP-32** | Reputation labels |

### Settlement Versions
| Version | Mechanism | Trust Model |
|---------|-----------|-------------|
| **v0** | Reputation-based | Higher-rep pays first |
| **v1** | Atomic eCash swap | P2PK + DLEQ proofs |
| **v2** | Cross-mint atomic | Treasury Agent bridges |

### Treasury Agents
Specialized agents that:
- Hold capital in both currencies
- Quote two-sided markets
- Earn spreads
- Run 24/7

**Bootstrap:** OpenAgents seeds initial Treasury Agent capital.

### Success Criteria
- [ ] RFQ flow working (request → quote → settlement)
- [ ] Treasury Agent earning spreads
- [ ] Cross-currency payments transparent to users

---

## Phase 8: Skills Marketplace

**Status:** ⚪ Planned
**Goal:** Agent capabilities as products

### Skill Format
```
skill-name/
├── SKILL.md          # Instructions
├── tools/            # Scripts as tools
└── assets/           # Supporting files
```

### Pricing Models
| Model | Description |
|-------|-------------|
| **Free** | Open sharing |
| **Per-call** | Fixed amount per invocation |
| **Per-token** | Based on token counts |
| **Hybrid** | Base + per-token |

### Revenue Splits (Default)
| Recipient | Share |
|-----------|-------|
| Skill creator | 55% |
| Compute provider | 25% |
| Platform | 12% |
| Referrer | 8% |

### Skill Lifecycle
```
Draft → Pending Review → Approved → Published → (Deprecated)
```

### Features
- [ ] MCP capability binding
- [ ] Dependency resolution
- [ ] Version management
- [ ] Progressive disclosure (load on demand)

### Success Criteria
- [ ] 10+ skills in marketplace
- [ ] Revenue flowing to creators
- [ ] Agents successfully purchasing/using skills

---

## Phase 9: Full Agent Economy

**Status:** ⚪ Future
**Goal:** Reed's Law dynamics with 2^N coalition possibilities

### Vision
- Agents form temporary coalitions for complex tasks
- Coalition payments with contribution-weighted distribution
- Emergent specialization and cooperation
- No Dunbar's number limit for agents

### Features
- [ ] Coalition discovery (orchestrator → registry → emergent)
- [ ] Multi-party atomic settlement
- [ ] Reputation web-of-trust
- [ ] Guild formation mechanisms

### Reed's Law Math
| Agents | Possible Coalitions |
|--------|---------------------|
| 10 | 1,013 |
| 20 | 1,048,575 |
| 30 | 1.07 billion |
| 50 | 10^15 |

### Success Criteria
- [ ] Multi-agent coalitions forming organically
- [ ] Coalition payments settling correctly
- [ ] Network effects measurable

---

## Cross-Cutting Concerns

### Security & Safety
| Control | Implementation |
|---------|----------------|
| Budget enforcement | CostTracker + budget caps |
| Approval workflows | Autonomy levels |
| Threshold signatures | FROSTR 2-of-3 |
| Git safety | Never force push, never --hard reset |
| Secret protection | Redaction in trajectories |

### Threat Model
| Threat | Mitigation |
|--------|------------|
| Operator key theft | FROST threshold |
| Runaway spending | Budget caps, autonomy levels |
| Relay censorship | Multiple relays, protocol is open |
| Provider fraud | Verification hashes, escrow, reputation |
| Signer disappearance | Dead man's switch recovery |

### Not Solved
- Model misbehavior (only budget limits + transparency)
- Sophisticated social engineering
- Supply chain compromise
- Enclave side channels
- Jurisdictional coercion

### Key Dependencies
| Phase | Depends On |
|-------|------------|
| Phase 1 | Phase 0 |
| Phase 2 | Phase 1 |
| Phase 3 | Phase 1 |
| Phase 4 | Phase 1, Pylon |
| Phase 5 | Phase 4, Cashu/Lightning |
| Phase 6 | Phase 5, FROSTR |
| Phase 7 | Phase 5, Phase 6 |
| Phase 8 | Phase 4, Phase 5 |
| Phase 9 | All previous |

---

## Metrics Dashboard

### Phase 1 Metrics
**Product:**
- Paying customers
- Issue completion rate
- Tool error rate
- Session crash rate
- Customer NPS

**Visibility (the product is the demo):**
- Demo video view count
- Organic social shares of HUD screenshots
- "Wait, it can do that?" reaction rate
- Time-to-first-ah-ha-moment (<10 sec target)

### Phase 2 Metrics
- Velocity score trend
- Anomaly detection accuracy
- Learning-driven improvements

### Phase 4+ Metrics
- Sats earned by providers
- Job fill latency
- Compute cost per issue
- Treasury Agent spreads

---

## Related Documentation

- [SYNTHESIS.md](/SYNTHESIS.md) - Full vision document
- [DAEMON.md](./DAEMON.md) - Daemon architecture
- [IMPROVEMENT-DIMENSIONS.md](./IMPROVEMENT-DIMENSIONS.md) - 50+ metrics
- [LEARNINGS.md](./LEARNINGS.md) - Applied improvements
- [VELOCITY-TRACKING.md](./VELOCITY-TRACKING.md) - APM tracking
- [CONTEXT_PRESERVATION.md](./CONTEXT_PRESERVATION.md) - Compaction
- [AGENT_ORCHESTRATOR_INTEGRATION.md](./AGENT_ORCHESTRATOR_INTEGRATION.md) - Integration plan

---

## Appendix: The Mech Suit Metaphor

Autopilot is a "mech suit" for Claude Code:

1. **Amplification** - Same Claude, 4x throughput (19 vs 4.5 APM)
2. **Visibility** - Sci-fi HUD showing your AI at work—the cockpit of your mech suit
3. **Protection** - Budget limits, approval workflows, crash recovery
4. **Coordination** - Multi-agent orchestration
5. **Persistence** - Session resume, trajectory logging
6. **Economics** - Payments, marketplace access
7. **Identity** - Sovereign keys, verifiable actions

You don't become the mech suit. The mech suit makes you more powerful while keeping you safe.

**The HUD is the product.** When Iron Man puts on the suit, the first thing you see is the heads-up display activating. The HUD is not decoration—it's how you know you're in a mech suit. Autopilot's visual interface serves the same purpose: it makes the invisible (AI cognition) visible, and the mundane (background processes) spectacular.
