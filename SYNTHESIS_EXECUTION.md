# OpenAgents: System Guide

A decentralized AI compute marketplace where agents run inference, earn Bitcoin, and operate autonomously. This document explains how the entire system works.

For the full vision, see [SYNTHESIS.md](./SYNTHESIS.md). For the agent OS concept, see [OANIX.md](./docs/OANIX.md).

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              OPENAGENTS STACK                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PRODUCTS (user-facing)                                                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐│
│  │   Coder     │ │    Onyx     │ │  GitAfter   │ │   openagents.com        ││
│  │ (AI coding) │ │  (editor)   │ │   (git)     │ │   (web dashboard)       ││
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └───────────┬─────────────┘│
│         │               │               │                     │              │
│         └───────────────┴───────────────┴─────────────────────┘              │
│                                   │                                          │
│  EXECUTION                        │                                          │
│  ┌────────────────────────────────┴───────────────────────────────────────┐ │
│  │  Adjutant (task execution) + Autopilot (autonomous loop)               │ │
│  │  DSPy decisions │ Claude SDK │ RLM/FRLM │ Local inference              │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                   │                                          │
│  INFRASTRUCTURE                   │                                          │
│  ┌─────────────┐ ┌────────────────┴──────────────┐ ┌────────────────────────┐│
│  │   Pylon     │ │         Runtime               │ │        WGPUI           ││
│  │(local node) │ │ Tick model │ FS abstraction   │ │   (GPU-rendered UI)    ││
│  │Provider/Host│ │ /compute │ /claude │ /wallet  │ │                        ││
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

The single binary for running sovereign AI agents. Supports two modes simultaneously:

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
- Claude (via claude-agent-sdk)

**Data directory:** `~/.openagents/pylon/`

---

### Coder — AI Coding Terminal

GPU-accelerated terminal interface for Claude Code. Built on wgpui for high-performance rendering.

```bash
cargo run -p coder
```

**Features:**
- Terminal-style interaction with Claude
- Autonomous autopilot loop (adjutant integration)
- MCP server management
- Command palette (`/help`, `/model`, `/session`, `/tools`)
- Rich Markdown rendering

The Coder integrates Adjutant for task execution with DSPy-powered decision making and self-improvement.

---

### Onyx — Markdown Editor

Local-first GPU-rendered Markdown note editor with live inline formatting.

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

```bash
cargo run -p gitafter
```

**Features:**
- Repository browser and issue tracking
- PR diff rendering with stacked diffs
- Lightning bounty metadata (NIP-57)
- Trajectory session links (agent work proof)
- Clone repositories locally

---

### Autopilot — Autonomous Coding Agent

The product layer for autonomous code tasks. Two deployment modes:

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

Nostr relay optimized for agent job coordination. Runs on Cloudflare Workers.

**Supported NIPs:**
- NIP-90: Data Vending Machines (job requests/results)
- NIP-89: Handler discovery
- NIP-42: Authentication (required)
- NIP-01: Basic protocol

**Job flow:**
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
└── claude/         # Claude SDK sessions
```

Works across: Browser (WASM), Cloudflare (DO), Local (SQLite), Server (Docker/K8s).

---

### Gateway — AI Provider Interface

Unified abstraction for AI service providers.

**Supported backends:**

| Backend | Endpoint | Detection |
|---------|----------|-----------|
| Ollama | localhost:11434 | Auto |
| llama.cpp | localhost:8080 | Auto |
| Apple FM | localhost:11435 | Auto (macOS) |
| Cerebras | API | Key required |
| Claude | API/CLI | SDK required |

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

Agent operating system runtime. Discovers environment at boot:

- Hardware detection (CPU, GPU, memory)
- Inference backend discovery (Ollama, FM Bridge, GPT-OSS)
- Network probing (relay connectivity)
- Identity loading

Produces an `OanixManifest` summarizing agent capabilities.

---

### Neobank — Treasury Management

Programmable treasury for agents:

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
2. **DelegationPipeline** — Route to claude_code, rlm, or local_tools
3. **RlmTriggerPipeline** — Decide if RLM is needed

**Execution priority:**
1. Claude Pro/Max (via claude-agent-sdk)
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
pylon rlm "Deep analysis" --backend claude
```

---

### LM Routing

Multi-backend routing via `lm-router`:

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
1. cargo autopilot run "Fix tests"
2. Preflight checks (config, auth, repo)
3. Adjutant makes DSPy decisions (complexity, delegation)
4. Execute via chosen path (Claude SDK, local, RLM)
5. Verify: cargo check + cargo test
6. Loop until success or max iterations
```

**Autopilot needs swarm inference:**
```
1. Runtime /compute/new → NIP-90 kind:5050
2. Nexus broadcasts to providers
3. Provider runs inference, sends invoice
4. Autopilot pays via /wallet/pay
5. Provider publishes result (kind:6050)
6. Runtime returns result
```

**Provider earns Bitcoin:**
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
| `coder` | GPU terminal for Claude Code |
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
| `claude-agent-sdk` | Rust SDK for Claude Code |
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

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

### Build Commands

```bash
# Products
cargo build --release -p pylon
cargo run -p coder
cargo run -p onyx
cargo run -p gitafter

# Autopilot
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
| Coder | Active | Autonomous loop with adjutant |
| Onyx | Alpha | Core editing works |
| GitAfter | v0.1 | NIP-34 integration |
| Nexus | v0.1 | NIP-90, NIP-42, NIP-89 |
| Runtime | In progress | Tick engine, filesystem |
| Adjutant | **Wave 14** | Self-improving autopilot |
| dsrs | **Wave 14** | Full DSPy implementation |
| WGPUI | Phase 16 | 377 tests, full component library |
| RLM | Working | Claude + Ollama backends |
| FRLM | Working | Distributed execution |
| Protocol | Complete | Job schemas, verification |
| Gateway | Complete | Multi-provider routing |
| Neobank | MVP | Treasury primitives |

**Bitcoin network:** Default is `regtest` for testing. Mainnet available.

---

## Quick Reference

```bash
# Start provider (earn sats)
pylon init && pylon start -f -m provider

# Run Coder
cargo run -p coder

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
