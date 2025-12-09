# Commander: Build Agents, Sell Compute, Earn Bitcoin

> **Sell your spare compute. Train agents that use the swarm. Get paid in bitcoin.** (Work in Progress)

---

## The Vision

Your device has idle compute. Millions of other devices do too. Commander aggregates this **stranded compute** into a swarm network. You sell your spare cycles, get paid in bitcoin straight to your built-in wallet.

Then you train agents that USE this network. Better agents = more demand for compute = more bitcoin for everyone.

**The Insight:** Edge AI on millions of idle devices will disrupt cloud AI economics faster than anyone expects. We're building the platform for that shift.

---

## How You Earn Bitcoin

### 1. Sell Your Compute

Your device sits idle most of the day. Commander lets you sell that spare compute to the swarm network:

- Click "Go Online" to start selling
- Other users' agents run inference on your hardware
- You get paid in bitcoin, straight to your wallet
- No middleman, no cloud provider taking a cut

### 2. Train Better Agents

Train **MechaCoder** through the **GYM** to excel at specific tasks:

- Agents that perform well get used more
- More usage = more demand for compute
- You can publish trained agents to the **Agent Store**
- When others use your agents, they pay for compute

### 3. Built-in Bitcoin Wallet

Commander includes a self-custodial bitcoin wallet:

- Receive payments for compute you sell
- Pay for compute when your agents need it
- Lightning Network for instant settlement
- Your keys, your bitcoin

---

## Core Components

### Swarm Compute Network

The aggregated compute of millions of devices:

```
YOUR DEVICE                    SWARM NETWORK
━━━━━━━━━━━                    ━━━━━━━━━━━━━
Idle CPU/GPU cycles    →       Pool of available compute
Apple Neural Engine    →       On-device inference jobs
Spare bandwidth        →       Agent-to-agent communication
```

**Why This Works:**
- Gavin Baker (hedge fund): "Edge AI is the scariest bear case for cloud AI capex"
- He thinks it takes 3 years. We think it's faster.
- Aggregated swarm compute changes the economics completely

### MechaCoder - Your First Agent

MechaCoder is an autonomous coding agent that:
- Picks tasks from your queue (`.openagents/tasks.jsonl`)
- Implements code following the Golden Loop v2 spec
- Runs tests to verify correctness
- Commits & pushes when tests pass
- Uses swarm compute for inference when needed

**Documentation:** [docs/mechacoder/](../mechacoder/)

### The GYM - Training Ground

Train agents to excel at specific skills:

- **Terminal-Bench** - Gold-standard benchmark for agent capabilities
- **MechaBench** - Custom challenges for specific skills
- **Training Plans** - Structured evaluation with objectives
- **Evolution** - Generate improved agent profiles from results

**The Three Curves:**

Our thesis - architecture beats raw model capability - reduces to three graphs:

1. **TestGen Score vs Evolution Step** - Does meta-learning work?
2. **HillClimber Pass Rate vs TestGen Config** - Does quality transfer?
3. **TB2 Performance vs Internal Metrics** - Is our proxy valid?

### FM Hill Climber - MAP Architecture

Solving Terminal-Bench with on-device Apple FM:

```
┌─────────────────────────────────────────────────────────────────┐
│                      MAP ORCHESTRATOR                           │
│  Coordinates modules, manages state, handles retry/backtrack    │
└─────────────────────────────────────────────────────────────────┘
           │           │           │           │
           ▼           ▼           ▼           ▼
    ┌───────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐
    │   TASK    │ │  ACTOR  │ │ MONITOR │ │EVALUATOR │
    │DECOMPOSER │ │  (FM)   │ │         │ │          │
    └───────────┘ └─────────┘ └─────────┘ └──────────┘
                       │
                ┌──────┴──────┐
                │  PARALLEL   │
                │  SAMPLER    │ ← Test-Time Compute
                └─────────────┘
```

**Key Achievement:** 89.5% on Terminal-Bench `regex-log` using only local FM inference.

**Documentation:** [docs/fm-hillclimber.md](../fm-hillclimber.md)

### Agent Store - Marketplace

Publish trained agents. Others use them on the swarm:

- Browse/search agents by category
- View Terminal-Bench scores and GYM metrics
- One-click install
- Agents consume swarm compute when they run

---

## The Economics

```
┌─────────────────────────────────────────────────────────────────┐
│                    THE COMPUTE FLYWHEEL                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│    SELL COMPUTE ──────► EARN BITCOIN ──────► TRAIN AGENTS      │
│         ▲                                         │             │
│         │                                         │             │
│         └────────── AGENTS USE COMPUTE ◄──────────┘             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**You earn when:**
- Your device processes inference jobs for the network
- Agents you trained get used (they pay for compute)
- You complete GYM bounties

**You spend when:**
- Your agents need more compute than your device provides
- You want to run training jobs faster

---

## Gamification

Commander is gamified with:

- **APM Tracking** - Measure actions per minute (StarCraft-inspired)
- **Trust Tiers** - Bronze → Silver → Gold progression
- **Skill Trees** - Unlockable agent capabilities
- **Achievements** - Milestones and challenges
- **Leaderboards** - Top compute sellers, best agent trainers

**Design Inspiration:**
- [StarCraft patterns](inspiration/starcraft.md) - APM, hotkeys, control groups
- [Factorio patterns](inspiration/factorio.md) - Factory management, production stats

---

## The Stakes

### If We Win Terminal-Bench #1 with Local Inference

**Industry Impact:**
- Proves architecture beats raw model capability
- Validates local-first AI over cloud dependency
- Positions swarm compute as viable alternative to data centers

**Platform Growth:**
- Commander becomes the client for swarm compute
- Agent Store becomes marketplace for AI skills
- Bitcoin becomes the native currency of edge AI

### The Paradigm Shift

```
CURRENT PARADIGM                  COMMANDER PARADIGM
━━━━━━━━━━━━━━━━                  ━━━━━━━━━━━━━━━━━
Cloud AI inference               Swarm compute on edge devices
Pay cloud providers              Get paid for your spare compute
Rent AI services                 Own and train your agents
Data leaves your device          Privacy-preserving local inference
```

---

## Technical Architecture

**Runtime:** Bun + Effect + TypeScript

**UI:** Effuse (Effect-native widgets) - [docs/effuse/](effuse/)

**Agents:** Apple FM (local), Claude Code (cloud fallback), MCP

**Payments:** Bitcoin Lightning, Spark

**Data:** SQLite, JSONL, Nostr

---

## Current State

- [x] MechaCoder core implementation
- [x] Golden Loop v2 spec
- [x] Task system (`.openagents/tasks.jsonl`)
- [x] FM Hill Climber (89.5% on regex-log)
- [ ] Swarm compute network integration
- [ ] Built-in bitcoin wallet
- [ ] Agent Store
- [ ] GYM with training plans

---

## Links

- [openagents.com](https://openagents.com)
- [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents)

---

**Last Updated:** 2025-12-09
**Status:** Work in Progress
