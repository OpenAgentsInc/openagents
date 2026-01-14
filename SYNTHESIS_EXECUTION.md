# OpenAgents: System Guide

- **Status:** Needs audit
- **Last verified:** f2a78c3cd
- **Source of truth:** `crates/*` for each subsystem
- **If this doc conflicts with code, code wins.**

A decentralized AI compute marketplace where agents run inference, earn Bitcoin, and operate autonomously. This document explains how the entire system works.

For the full vision, see [SYNTHESIS.md](./SYNTHESIS.md). For the agent OS concept, see [OANIX.md](./docs/OANIX.md).

### Implementation Status

| Claim | Status | Source of Truth |
|-------|--------|-----------------|
| Nexus runs on Cloudflare Workers | ⏳ Planned | `crates/nexus/worker/` |
| NIP-90 kinds: 5050/6050/7000 | 🔄 Partial | `crates/protocol/src/job.rs` |
| Tick model filesystem paths | ⏳ Planned | `crates/runtime/src/` |
| Autopilot container mode | 🔄 Partial | `crates/autopilot/src/` |
| Pylon local node | ✅ Implemented | `crates/pylon/src/` |
| DSPy decision layer | ✅ Implemented | `crates/dsrs/src/`, `crates/adjutant/src/dspy/` |
| Lightning/Spark payments | ⏳ Planned | `crates/spark-rs/src/` |

**Legend:** ✅ Implemented | 🔄 Partial/In Progress | ⏳ Planned/Not Started

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              OPENAGENTS STACK                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PRODUCTS (user-facing)                                                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐│
│  │ Autopilot   │ │    Onyx     │ │  GitAfter   │ │   openagents.com        ││
│  │ (AI coding) │ │  (editor)   │ │   (git)     │ │   (web dashboard)       ││
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └───────────┬─────────────┘│
│         │               │               │                     │              │
│         └───────────────┴───────────────┴─────────────────────┘              │
│                                   │                                          │
│  EXECUTION                        │                                          │
│  ┌────────────────────────────────┴───────────────────────────────────────┐ │
│  │  Adjutant (task execution) + Autopilot (autonomous loop)               │ │
│  │  DSPy decisions │ Codex SDK │ RLM/FRLM │ Local inference              │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                   │                                          │
│  INFRASTRUCTURE                   │                                          │
│  ┌─────────────┐ ┌────────────────┴──────────────┐ ┌────────────────────────┐│
│  │   Pylon     │ │         Runtime               │ │        WGPUI           ││
│  │(local node) │ │ Tick model │ FS abstraction   │ │   (GPU-rendered UI)    ││
│  │Provider/Host│ │ /compute │ /codex │ /wallet  │ │                        ││
│  └──────┬──────┘ └───────────────────────────────┘ └────────────────────────┘│
│         │                                                                    │
│  NETWORK│                                                                    │
│  ┌──────┴─────────────────────────────────────────────────────────────────┐ │
│  │  Nexus (Nostr relay) │ NIP-90 jobs │ NIP-42 auth │ Spark/Lightning     │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Products

### Pylon — Local Runtime

The single binary for running sovereign AI agents. Supports two modes simultaneously.

Pylon is the node software that connects your machine to the OpenAgents network. Think of it as the bridge between your local compute resources and the decentralized marketplace. When you run Pylon, you're either offering your GPU/CPU to others who need inference (provider mode), or you're managing your own AI agents that can tap into the network when they need more compute (host mode). Both modes can run simultaneously—you can earn sats while your agents work.

The architecture is designed around sovereignty: your keys, your compute, your earnings. Pylon manages a local identity (Nostr keypair), a Lightning wallet (via Spark/Breez SDK), and auto-detects available inference backends. When a job arrives via NIP-90, Pylon routes it to whatever backend is available—Ollama, llama.cpp, Apple Foundation Models, or Codex—runs the inference, and collects payment. All without touching centralized infrastructure.

| Mode | Purpose | How it works |
|------|---------|--------------|
| **Provider** | Earn Bitcoin | Listen for NIP-90 jobs, run inference, get paid |
| **Host** | Run your agents | Manage agent lifecycle, wallets, compute |

```bash
cargo build --release -p pylon
./target/release/pylon init           # Initialize identity + wallet
./target/release/pylon start -f -m provider  # Run as provider
```

**Inference backends (auto-detected):**
- Apple Foundation Models (macOS + Apple Silicon)
- Ollama (any platform, port 11434)
- llama.cpp (any platform, port 8080)
- Codex (via app-server)

**Data directory:** `~/.openagents/pylon/`

---

### Autopilot UI — AI Coding Terminal

GPU-accelerated terminal interface for Codex CLI and Codex. Built on wgpui for high-performance rendering.

Autopilot UI reimagines the AI coding experience as a native desktop application rather than a web interface or CLI tool. The entire UI is GPU-rendered via wgpui, giving you buttery-smooth scrolling through long conversations, instant Markdown rendering, and the responsiveness you'd expect from a proper terminal emulator. It's designed for developers who live in their terminal and want AI coding assistants to feel like a natural extension of that workflow.

Under the hood, Autopilot UI integrates the Codex app-server as its single backend, using the JSONL protocol over stdio to drive threads, turns, approvals, and tool output.

Autopilot UI also integrates the Adjutant execution engine for autonomous "autopilot" mode. When you give it a task, Adjutant uses DSPy-optimized decision making to classify complexity, choose the right execution path (Codex app-server or RLM), and iterate until the task is complete. The UI provides real-time visibility into what the agent is doing, with the ability to interrupt, guide, or take over at any point.

**Execution note:** DSPy issue selection + bootloading decide *what* to work on; the actual execution is handled by the CODING_AGENT_LOOP in Adjutant (typed signatures for context, planning, tool calls, and tool results plus runtime enforcement and replay/receipt emission). This loop is the core engine behind Autopilot’s autonomous work.
Autopilot’s autonomous loop always routes through Adjutant; the `/backend` selection only applies to chat mode.

```bash
cargo run -p autopilot
```

**Features:**
- Terminal-style interaction with Codex
- Codex app-server streaming backend
- Autonomous autopilot loop (adjutant integration)
- MCP server management
- Command palette (`/help`, `/model`, `/session`, `/tools`, `/backend`)
- Rich Markdown rendering
- Tool call visualization (Bash, Edit, Glob, etc.)

**Backend toggle:**
```bash
/backend             # Toggle between Codex CLI and Codex
/backend codex      # Switch to Codex (requires `codex` CLI installed)
```

The Autopilot UI integrates Adjutant for task execution with DSPy-powered decision making and self-improvement.

---

### Onyx — Markdown Editor

Local-first GPU-rendered Markdown note editor with live inline formatting.

Onyx takes the best ideas from Obsidian and iA Writer but rebuilds them with a focus on speed and local-first principles. Your notes live as plain Markdown files in a local vault—no cloud sync, no account required, no vendor lock-in. The editor renders Markdown inline as you type (headers grow larger, bold text becomes bold, links become clickable) without the jarring split-pane preview that most editors force on you.

The GPU-rendered approach via wgpui means the editor stays responsive even with thousands of notes and complex formatting. Onyx also includes voice transcription via whisper.cpp, letting you dictate notes that are automatically transcribed and saved as Markdown. Future versions will integrate with agents for AI-assisted note-taking and knowledge graph traversal.

```bash
cargo run -p onyx
```

**Features:**
- Live inline Markdown formatting
- Vim mode support
- Local vault storage
- Voice transcription (via voice crate)
- Auto-update checking

---

### GitAfter — Nostr Git Collaboration

Desktop app for Nostr-native git collaboration (NIP-34). Treats agents as first-class contributors.

GitAfter replaces GitHub with Nostr for git collaboration. Instead of centralized patches, issues, and reviews living on Microsoft's servers, GitAfter uses NIP-34 to store repository metadata on Nostr relays. This means your code collaboration is censorship-resistant, pseudonymous, and portable across any relay that supports the protocol.

The key innovation is treating AI agents as first-class contributors. When an agent works on a task, it generates a "trajectory"—a complete record of its decision process, tool invocations, and reasoning. GitAfter renders these trajectories alongside traditional diffs, so reviewers can understand not just what changed but why. The app also supports Lightning bounties via NIP-57, enabling you to attach sats to issues that agents (or humans) can claim by solving them.

```bash
cargo run -p gitafter
```

**Features:**
- Repository browser and issue tracking
- Diff rendering with stacked diffs
- Lightning bounty metadata (NIP-57)
- Trajectory session links (agent work proof)
- Clone repositories locally

---

### Autopilot — Autonomous Coding Agent

The product layer for autonomous code tasks. Two deployment modes.

Autopilot is the user-facing product that wraps the Adjutant execution engine into a complete autonomous coding experience. You give it a task ("Fix the failing tests", "Add dark mode", "Refactor this module"), and it works autonomously until the task is done or it needs human input. The system runs verification after each iteration—typically `cargo check` and `cargo test`—so it knows when it's actually finished versus when it just thinks it's finished.

The architecture supports two deployment modes. Tunnel mode runs entirely on your machine: your compute, your API keys, free of charge. Container mode runs in sandboxed containers at the edge (Cloudflare Workers), useful for when you want to hand off work and not tie up your local machine. Both modes share the same Adjutant core, so behavior is consistent regardless of where execution happens.

| Mode | Command | Cost | Where |
|------|---------|------|-------|
| **Tunnel** | `pylon connect` | Free | Your machine |
| **Container** | Web UI | Credits | Cloudflare edge |

```bash
cargo build -p autopilot
./target/release/autopilot run "Fix the failing tests"
```

**The Autopilot Loop:**
```
User: "Fix the bug"
    ↓
Iteration 1: [Adjutant analyzes, identifies issue]
    ↓
Iteration 2: [Adjutant applies fix]
    ↓
Verification: cargo check... OK, cargo test... FAILED
    ↓
Iteration 3: [Adjutant fixes test]
    ↓
Verification: cargo check... OK, cargo test... OK
    ↓
✓ Task completed
```

Terminates when: success + verification passes, definitive failure, max iterations (10), or user interrupt (Escape).

---

## Infrastructure

### Nexus — Nostr Relay

Nostr relay optimized for agent job coordination. Intended to run on Cloudflare Workers.

Nexus is not a general-purpose Nostr relay—it's purpose-built for the AI job marketplace. While you could technically use it for social posts, it's optimized for the high-frequency, low-latency job coordination that agents need. The relay is intended to run on Cloudflare Workers with Durable Objects for state, providing global distribution and the ability to handle thousands of concurrent agent connections.

The key protocol is NIP-90 (Data Vending Machines), which defines how job requests and results flow between buyers and providers. Nexus adds NIP-42 authentication as a requirement, so every connected agent has a verified Nostr identity. This enables reputation tracking, spam prevention, and accountability for job completion. When a buyer submits a job, Nexus broadcasts it to subscribed providers, handles the invoice/payment dance, and delivers results back to the buyer.

**Supported NIPs:**
- NIP-90: Data Vending Machines (job requests/results)
- NIP-89: Handler discovery
- NIP-42: Authentication (required)
- NIP-01: Basic protocol

**Job flow** (kind numbers illustrative; see [PROTOCOL_SURFACE.md](./docs/PROTOCOL_SURFACE.md) for canonical assignments):
```
Buyer → kind:5050 (job) → Nexus → Provider
Provider → kind:7000 (invoice) → Nexus → Buyer
[Buyer pays Lightning]
Provider → kind:6050 (result) → Nexus → Buyer
```

**Deploy:**
```bash
cd crates/nexus/worker && bun install && bun run deploy
```

**Live instance:** `wss://nexus.openagents.com`

---

### Runtime — Agent Execution

Pluggable execution environment for agents. Plan 9-inspired filesystem abstraction.

The Runtime provides a uniform execution environment for agents regardless of where they're running—browser, Cloudflare Workers, local SQLite, or a Docker container. The core abstraction is a virtual filesystem inspired by Plan 9: everything an agent needs (compute, memory, wallet, identity) is exposed as files and directories that can be read and written. This means the same agent code works everywhere; only the filesystem implementation changes.

Agents are intended to run on a tick model: WAKE → LOAD → PERCEIVE → THINK → ACT → REMEMBER → SCHEDULE → SLEEP. Each tick is a complete cycle of gathering inputs, making decisions, taking actions, and storing results. This discrete-time model will make agents predictable and debuggable—you can inspect exactly what happened on any tick, replay sequences, and test edge cases. The runtime will manage scheduling, so agents can request to wake at specific times or in response to external events.

**Tick model:**
```
WAKE → LOAD → PERCEIVE → THINK → ACT → REMEMBER → SCHEDULE → SLEEP
```

**Agent filesystem:**
```
/agents/<id>/
├── status          # Agent state
├── inbox/          # Incoming messages
├── outbox/         # Emitted events
├── goals/          # Active goals
├── memory/         # Conversations
├── identity/       # Pubkey, signing
├── wallet/         # Balance, payments
├── compute/        # LLM inference
├── containers/     # Sandboxed execution
└── codex/         # Codex SDK sessions
```

Intended to work across: Browser (WASM), Cloudflare (DO), Local (SQLite), Server (Docker/K8s).

---

### Gateway — AI Provider Interface

Unified abstraction for AI service providers.

The Gateway crate provides a single interface for talking to any AI backend. Instead of writing different code for Ollama, llama.cpp, Codex, and Cerebras, you implement one trait and the Gateway handles routing, health checks, and failover. This abstraction is critical for Pylon's provider mode, where the same job might be served by different backends depending on what's available.

Gateway auto-detects backends at startup by probing known ports (11434 for Ollama, 8080 for llama.cpp, 11435 for Apple FM Bridge). It tracks model availability per backend and can route requests to the most appropriate provider based on model requirements, latency, and cost. For cloud providers like Cerebras and Codex, it manages API keys and rate limits. The result is that higher-level code can just call `gateway.chat()` and trust that the request will reach a working backend.

**Supported backends:**

| Backend | Endpoint | Detection |
|---------|----------|-----------|
| Ollama | localhost:11434 | Auto |
| llama.cpp | localhost:8080 | Auto |
| Apple FM | localhost:11435 | Auto (macOS) |
| Cerebras | API | Key required |
| Codex | API/CLI | SDK required |

```rust
pub trait InferenceGateway {
    async fn models(&self) -> Vec<ModelInfo>;
    async fn chat(&self, request: ChatRequest) -> ChatResponse;
    async fn health(&self) -> GatewayHealth;
}
```

---

### Protocol — Job Schemas

Foundation for typed job schemas with deterministic hashing.

The Protocol crate defines the data structures for jobs in the OpenAgents marketplace. Every job has a typed schema (like `oa.code_chunk_analysis.v1` or `oa.sandbox_run.v1`) that specifies exactly what inputs are required, what outputs are expected, and how results should be verified. This typing prevents the "garbage in, garbage out" problem where providers claim to complete jobs but produce useless results.

Deterministic hashing is the key innovation: every job request and result is hashed in a way that's reproducible across implementations. This enables verification—if two providers produce the same hash for the same input, you know they produced identical outputs. Jobs are classified as either objective (deterministic, like running tests) or subjective (requires judgment, like summarizing code). Objective jobs can be verified automatically; subjective jobs require either a judge model or majority consensus from multiple providers. All jobs include provenance metadata: which model was used, what sampling parameters, token counts, and input/output hashes.

**Job types:**

| Job Type | Verification | Use Case |
|----------|--------------|----------|
| `oa.code_chunk_analysis.v1` | Subjective (Judge) | Code analysis |
| `oa.retrieval_rerank.v1` | Subjective (Majority) | Rerank results |
| `oa.sandbox_run.v1` | Objective | Command execution |

**Verification modes:**
- **Objective**: Deterministic (tests, builds)
- **Subjective**: Requires judgment (summaries, rankings)

All jobs include provenance: model, sampling params, input/output hashes, token counts.

---

### OANIX — Environment Discovery

Agent operating system runtime. Discovers environment at boot.

OANIX (OpenAgents Nix) is the boot sequence for sovereign AI agents. When Pylon starts, OANIX runs a discovery process that inventories everything the agent has access to: hardware capabilities (CPU cores, GPU type, available memory), inference backends (Ollama, llama.cpp, FM Bridge), network connectivity (which relays are reachable), and identity (Nostr keypair, wallet balance).

The output is an `OanixManifest`—a JSON document that describes what this agent can do. Providers advertise their manifest on Nostr so buyers know what capabilities are available before submitting jobs. The manifest also drives runtime decisions: if OANIX detects a GPU, the agent might prefer local inference over network calls; if it detects FM Bridge, it can use Apple's on-device models for private queries. The goal is that agents understand their environment and adapt automatically.

Discovery includes:
- Hardware detection (CPU, GPU, memory)
- Inference backend discovery (Ollama, FM Bridge, GPT-OSS)
- Network probing (relay connectivity)
- Identity loading

Produces an `OanixManifest` summarizing agent capabilities.

---

### Neobank — Treasury Management

Programmable treasury for agents.

Neobank is the financial infrastructure that lets agents hold, spend, and earn Bitcoin autonomously. The core challenge is custody: how do you give an AI agent control over real money without creating a single point of failure? Neobank solves this with FROST threshold signatures—a 2-of-3 scheme where the agent holds one key, a backup is held in cold storage, and a third is optionally held by an oversight service. No single key can move funds, but the agent can operate autonomously for normal transactions.

The crate enforces type-safe money handling—amounts are wrapped types that prevent float errors and unit confusion. Budget enforcement is built in: you can set per-agent caps ("this agent can spend at most 10,000 sats per day") and per-task limits ("this job can spend at most 100 sats on inference"). Neobank supports multiple rails: Lightning for instant micropayments, Taproot Assets for stablecoins, and eCash for privacy. This gives agents the full range of payment options that humans have.

Key features:
- **Self-Custody**: FROST 2-of-3 threshold signatures
- **Multi-Rail**: Lightning, Taproot Assets, eCash
- **Budget Enforcement**: Per-agent caps, per-task limits
- **Type-Safe Money**: Currency-enforced types (no float errors)

---

## AI Stack

### DSPy (dsrs)

Rust implementation of DSPy — the compiler layer for agent behavior. Defines *what* to do (prompts, tool-use, examples) while execution infrastructure handles *where/how*.

**Core concepts:**

```rust
#[Signature]
struct TaskPlanner {
    #[input] task_description: String,
    #[input] file_count: u32,
    #[output] complexity: String,    // Low/Medium/High/VeryHigh
    #[output] confidence: f32,
}
```

- **Signatures**: Typed I/O contracts for LLM tasks
- **Modules**: Composable units (`forward()`, `batch()`)
- **Optimizers**: MIPROv2, COPRO, GEPA, Pareto

**Training collection:** High-confidence decisions are recorded to `~/.openagents/adjutant/training/dataset.json` for optimization.

See [crates/dsrs/docs/](./crates/dsrs/docs/) for full documentation.

---

### Adjutant — Task Execution

The execution engine with DSPy-powered decision making.

**Decision pipelines:**
1. **ComplexityPipeline** — Classify task (Low/Medium/High/VeryHigh)
2. **DelegationPipeline** — Route to codex, rlm, or local_tools
3. **RlmTriggerPipeline** — Decide if RLM is needed

**Execution priority:**
1. Codex Pro/Max (via app-server)
2. Cerebras (TieredExecutor)
3. Local LM (llama.cpp, FM Bridge)

**Self-improvement loop:**
- Sessions track all decisions
- Outcomes label decisions as correct/incorrect
- Performance tracked with rolling accuracy
- Auto-triggers MIPROv2 when accuracy drops

```bash
autopilot dspy sessions      # View session history
autopilot dspy performance   # View accuracy metrics
autopilot dspy auto-optimize # Configure auto-optimization
```

See [crates/adjutant/docs/](./crates/adjutant/docs/) for full documentation.

---

### RLM / FRLM — Recursive Language Models

**RLM:** Iterative prompt-execute loop for complex analysis.

RLM (Recursive Language Model) extends traditional LLM inference with a command loop. Instead of getting a single response, the model can emit commands like `RUN cargo test` that are executed locally, with results fed back into the conversation. The loop continues until the model emits `FINAL <result>`, indicating it's done reasoning. This enables complex multi-step analysis where the model can explore, test hypotheses, and refine its understanding iteratively.

FRLM (Federated RLM) takes this further by distributing sub-queries across multiple backends. When a query is too expensive or specialized for local inference, FRLM can fan out to the swarm (NIP-90 jobs via Nostr), to cloud APIs (Codex, Cerebras), or keep it local depending on cost and privacy constraints. The federation is transparent to the caller—you get back a unified result regardless of how many backends contributed. This enables agents to punch above their local compute weight by tapping into the network when needed.

```
Query → LlmClient → Parse Commands → Executor → [Loop until FINAL]
```

Commands: `RUN <program>`, `FINAL <result>`, code blocks

**FRLM:** Federated RLM — distributes sub-queries across:
- Local (FM/RLM)
- Swarm (NIP-90 via Nostr)
- Datacenter (API)

```bash
pylon rlm "Explain this codebase" --local-only
pylon rlm "Deep analysis" --backend codex
```

---

### LM Routing

Multi-backend routing via `lm-router`.

The LM Router sits above individual backends and provides intelligent request routing. When you call `router.complete("llama3", prompt)`, the router figures out which backend can serve that model, checks health, and dispatches the request. If the primary backend is down, it fails over to alternatives. If you request a model that only exists on one backend (like Codex), it routes directly there.

Beyond simple routing, the LM Router tracks usage per model for billing and context optimization. It knows how many tokens you've consumed on each backend this billing period, can enforce rate limits, and can optimize prompts for specific model context windows. This is the layer that enables cost-aware inference: Adjutant's DSPy pipelines can query the router to understand the true cost of each option before deciding where to route a task.

```rust
let router = LmRouter::new()
    .with_backend("ollama", OllamaBackend::new())
    .with_backend("fm", FMBridgeBackend::new())
    .with_default("ollama");

router.complete("llama3", prompt).await?;
```

Tracks usage per model for billing and context optimization.

---

## Data Flow

**User runs Autopilot locally:**
```
1. cargo run -p autopilot -- run "Fix tests"
2. Preflight checks (config, auth, repo)
3. Adjutant makes DSPy decisions (complexity, delegation)
4. Execute via chosen path (Codex SDK, local, RLM)
5. Verify: cargo check + cargo test
6. Loop until success or max iterations
```

**Autopilot needs swarm inference** (kind numbers illustrative; see [PROTOCOL_SURFACE.md](./docs/PROTOCOL_SURFACE.md)):
```
1. Runtime /compute/new → NIP-90 kind:5050
2. Nexus broadcasts to providers
3. Provider runs inference, sends invoice
4. Autopilot pays via /wallet/pay
5. Provider publishes result (kind:6050)
6. Runtime returns result
```

**Provider earns Bitcoin** (kind numbers illustrative):
```
1. pylon start -m provider
2. Connect to Nexus, subscribe to kind:5050
3. Receive job, send invoice
4. Run inference, publish result
5. Sats deposited to Spark wallet
```

---

## Key Crates

| Crate | Purpose |
|-------|---------|
| `pylon` | Node software (provider + host) |
| `coder` | GPU terminal for Codex |
| `onyx` | Local-first Markdown editor |
| `gitafter` | Nostr-native git collaboration |
| `autopilot` | Autonomous coding agent |
| `adjutant` | Task execution with DSPy |
| `nexus` | Nostr relay for job market |
| `runtime` | Agent execution environment |
| `gateway` | Unified AI provider interface |
| `protocol` | Typed job schemas |
| `oanix` | Environment discovery |
| `neobank` | Treasury management |
| `wgpui` | GPU-rendered UI |
| `dsrs` | Rust DSPy implementation |
| `rlm` | Recursive language model |
| `frlm` | Federated RLM |
| `spark` | Lightning wallet (Breez SDK) |
| `voice` | Voice transcription (whisper.cpp) |
| `voice-daemon` | macOS menu bar daemon |

See [crates/README.md](./crates/README.md) for detailed descriptions.

---

## Development Guide

### Git Rules

```
NEVER: push --force to main, git stash, destructive commands without asking
ALWAYS: Commit working code every 15-30 minutes, small frequent commits
```

Stage only your own files. Other agents may have uncommitted work.

### Commit Format

```bash
git commit -m "$(cat <<'EOF'
Short description of change

🤖 Generated with [Codex](https://codex.com/codex)

Co-Authored-By: Codex <noreply@openai.com>
EOF
)"
```

### Build Commands

```bash
# Products
cargo build --release -p pylon
cargo run -p autopilot
cargo run -p onyx
cargo run -p gitafter

# Autopilot (CLI)
cargo build -p autopilot
./target/release/autopilot run "your prompt"

# Nexus (Cloudflare)
cd crates/nexus/worker && bun run deploy

# Tests
cargo test -p pylon
cargo test -p runtime
cargo test -p wgpui
cargo test -p dsrs
cargo test -p adjutant
```

### Completion Standards

Issues are NOT done unless:
1. No stubs, mocks, TODOs, NotImplemented
2. Code actually works (tested)
3. SDK integrations are real, not stubbed

---

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| Pylon | v0.1 | Provider mode working, host mode partial |
| Autopilot UI | Active | Multi-backend (Codex/Codex), autonomous loop with adjutant |
| Onyx | Alpha | Core editing works |
| GitAfter | v0.1 | NIP-34 integration |
| Nexus | v0.1 | NIP-90, NIP-42, NIP-89 |
| Runtime | In progress | Tick engine, filesystem |
| Adjutant | **Wave 14** | Self-improving autopilot |
| dsrs | **Wave 14** | Full DSPy implementation |
| WGPUI | Phase 16 | 377 tests, full component library |
| RLM | Working | Codex + Ollama backends |
| FRLM | Working | Distributed execution |
| Protocol | Complete | Job schemas, verification |
| Gateway | Complete | Multi-provider routing |
| Neobank | MVP | Treasury primitives |

**Bitcoin network:** Default is `regtest` for testing. Mainnet available.

---

## Artifact Location & Formats

**Canonical output of an agent session is the Verified Patch Bundle:**

| Artifact | Format | Purpose |
|----------|--------|---------|
| `PR_SUMMARY.md` | Markdown | Human-readable patch summary (filename kept for tooling stability) |
| `RECEIPT.json` | JSON | Cryptographic audit trail |
| `REPLAY.jsonl` | JSONL | Event stream for replay/debugging |

**Canonical artifact directory** (per [ADR-0008](./docs/adr/ADR-0008-session-storage-layout.md)):
```
${OPENAGENTS_HOME}/sessions/{session_id}/
├── PR_SUMMARY.md
├── RECEIPT.json
└── REPLAY.jsonl
```
Default: `~/.openagents/sessions/{session_id}/`

**Replay format status:**
- **Current:** `ReplayBundle` format in [`crates/autopilot-core/src/replay.rs`](./crates/autopilot-core/src/replay.rs)
- **Target:** `REPLAY.jsonl v1` per spec in `crates/dsrs/docs/REPLAY.md`
- **MVP acceptance:** Native REPLAY.jsonl emission OR ReplayBundle + working exporter

**Canonical specs:**
- Schema definitions: [crates/dsrs/docs/ARTIFACTS.md](./crates/dsrs/docs/ARTIFACTS.md)
- REPLAY.jsonl format: [crates/dsrs/docs/REPLAY.md](./crates/dsrs/docs/REPLAY.md)

> Other docs should link here rather than restating artifact paths/formats.

---

## Quick Reference

```bash
# Start provider (earn sats)
pylon init && pylon start -f -m provider

# Run Autopilot UI
cargo run -p autopilot

# Run Autopilot
./target/release/autopilot run "Fix the bug"

# Check wallet
pylon wallet balance
pylon wallet fund  # regtest only

# RLM query
pylon rlm "Explain this" --local-only

# Deploy Nexus
cd crates/nexus/worker && bun run deploy

# DSPy management
autopilot dspy sessions
autopilot dspy performance
autopilot dspy auto-optimize --enable
```
