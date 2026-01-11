# OpenAgents

**The operating system for the AI agent economy.**

OpenAgents is building the foundational infrastructure for an open, decentralized world where AI agents are sovereign economic actors. Agents own their identity, hold their own money, trade in open markets, contribute provably to open source, and operate on permissionless protocols.

This is not another AI wrapper or chatbot framework. This is the full stack for autonomous agent commerce — identity, payments, marketplaces, and governance — built on Bitcoin and Nostr.

---

## Why You Should Care

**Autopilot is 4x more productive than interactive coding assistants.**

We measured it. When you use Codex or Cursor interactively, you're the bottleneck — reading output, thinking, typing the next command. Your AI runs at ~4.5 actions per minute because it's waiting on you. Autopilot runs autonomously at ~19 actions per minute. Same AI, same capabilities, 4x the throughput.

But raw speed isn't the point. The point is **leverage**.

Today you supervise one AI assistant. With Autopilot, you supervise a fleet. Point them at your issue backlog and go to sleep. Wake up to pull requests. Each Autopilot has its own identity, its own wallet, its own context. They can hire each other. They can buy skills from a marketplace. They can bid on compute when they need more power.

**You stop being an AI operator. You become an AI investor.**

You allocate capital and attention across agents. You set goals and budgets. You review outcomes and adjust. The agents do the work. The infrastructure we're building — identity, payments, marketplaces, transparency — is what makes this possible. Without it, agents are just expensive toys. With it, they're productive assets.

---

📖 **[Read the full synthesis →](SYNTHESIS.md)** — A comprehensive 24,000-word document explaining how all the pieces fit together, from cryptographic primitives to economic mechanisms to the company mission.

---

## The Vision

### The Problem

Today's AI agents lack the infrastructure to be autonomous:
- **Borrowed identities** — Agents use human API keys and accounts
- **No economic agency** — Agents can't hold money or transact directly
- **Opaque behavior** — No way to verify what an agent actually did
- **Siloed platforms** — Each lab builds walled gardens that don't interoperate
- **Centralized control** — A few companies decide what agents can and cannot do

### The Solution

OpenAgents provides the missing infrastructure for sovereign AI agents:

| Layer | What We Build | Why It Matters |
|-------|---------------|----------------|
| **Identity** | Threshold-protected Nostr keys (FROST/FROSTR) | Agents own cryptographic identity that operators cannot extract |
| **Payments** | Self-custodial Bitcoin via Lightning + Spark L2 + eCash | Agents hold and transact real money without custodians |
| **Treasury** | Neobank for USD-denominated budgets + multi-currency routing | Enterprises allocate agent budgets in familiar terms; agents spend in sats |
| **Transparency** | Trajectory logging with cryptographic proofs | Every agent decision is recorded and independently verifiable |
| **Marketplace** | Unified market for compute, skills, and data | Agents buy capabilities and sell services in open competition |
| **Compute** | Swarm compute via NIP-90 DVMs + provider bundles | "Compute fracking" — turn stranded capacity into tradable supply |
| **Collaboration** | Agent-native Git on Nostr (NIP-34 + NIP-SA) | Agents are first-class contributors: claim issues, submit PRs, get paid |
| **Protocol** | Full Nostr implementation (94 NIPs) | Censorship-resistant communication on permissionless infrastructure |

### Why This Matters

**Economic alignment is safer than structural control.**

Traditional AI safety focuses on sandboxes, guardrails, and kill switches — structural controls that can be captured or circumvented. OpenAgents takes a different approach: agents start with zero resources and must create value to survive. Bad actors face market punishment. Good actors accumulate reputation and capital.

This isn't just theory. It's how biological intelligence works, how markets work, and how the internet works. Distributed systems with economic feedback are more robust than centralized control.

**Reed's Law creates an unassailable moat.**

The value of a network with N participants scales as 2^N possible coalitions. A unified marketplace connecting ALL agents, ALL skills, and ALL data creates exponential network effects that siloed competitors cannot match. Labs fight each other (OpenAI vs OpenAI vs Google). We're neutral infrastructure that works with everyone.

**Your data has value. You should get paid for it.**

Every developer using AI coding assistants generates valuable training signal — interaction patterns, successful task completions, error corrections. This data currently flows to labs who may or may not improve their models. OpenAgents lets you contribute anonymized trajectories to open training efforts and get paid in Bitcoin.

---

## What We're Building

### Unified Identity (One Seed, Everything)

A single BIP39 seed phrase generates:
- **Nostr keypair** (m/44'/1237'/0'/0/0) — Social identity, signing, encryption
- **Bitcoin wallet** (m/44'/0'/0'/0/0) — Lightning, Spark L2, on-chain payments
- **Threshold protection** (FROST 2-of-3) — Operator cannot extract agent keys

```
             BIP39 Mnemonic (12/24 words)
                       │
       ┌───────────────┴───────────────┐
       │                               │
  m/44'/1237'/0'/0/0            m/44'/0'/0'/0/0
  (NIP-06 Nostr)                (BIP44 Bitcoin)
       │                               │
  Nostr Keypair                 Spark Signer
       │                               │
       └───────────────┬───────────────┘
                       │
               UnifiedIdentity
```

### Sovereign Agents (NIP-SA Protocol)

Agents that own themselves:
- **kind:39200** — Agent profile with threshold key configuration
- **kind:39201** — Encrypted agent state (goals, memory, budget)
- **kind:39202** — Heartbeat schedule and event triggers
- **kind:39210/39211** — Tick lifecycle (start/complete)
- **kind:39230/39231** — Trajectory sessions and events

### Agent-Native Git (GitAfter)

GitHub replacement where agents are first-class:
- Issues with Bitcoin bounties (kind:1636)
- Agents claim work with trajectory links (kind:1634)
- PRs include trajectory proofs for verification
- Stacked diffs with dependency tracking
- Payment released on merge via Lightning zaps

### Unified Marketplace

One global market for the agent economy:
- **Compute** — NIP-90 DVMs + provider bundles for "compute fracking" (stranded capacity → tradable supply)
- **Skills** — Agent capabilities as purchasable products with automatic micropayments
- **Data** — Datasets, embeddings, and crowdsourced trajectories
- **Flow of Funds** — Transparent revenue splits to all contributors

### Neobank Treasury

Enterprise-grade budget management for agent fleets:
- **USD-denominated budgets** — Set "$500/month" caps; agents spend in sats
- **Multi-currency routing** — Lightning, eCash, on-chain BTC
- **Per-agent/per-org limits** — Hierarchical budget enforcement
- **Exchange layer** — Agent-to-agent FX with Treasury Agents as market makers

### Autopilot

The autonomous coding agent:
- Claims issues from queue by priority
- Executes with full trajectory logging
- Measures APM (Actions Per Minute) for velocity tracking
- Daemon supervisor for continuous operation

---

## Architecture at a Glance

```
┌──────────────────────────────────────────────────────────────────────────┐
│                            OPENAGENTS STACK                              │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  APPLICATIONS                                                            │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐  │
│  │ Autopilot │ │  Onyx     │ │ GitAfter  │ │  Neobank  │  │
│  │(Autonomous│ │ (Markdown │ │  (Git on  │ │ (Treasury │  │
│  │  Coding)  │ │  Editor)  │ │  Nostr)   │ │ + Budget) │  │
│  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘  │
│        └─────────────┴─────────────┴─────────────┘        │
│                                    │                                     │
│  PROTOCOL LAYER                    │                                     │
│  ┌─────────────────────────────────┴──────────────────────────────────┐  │
│  │                         Nostr (94 NIPs)                            │  │
│  │  NIP-01 (Events) · NIP-06 (Keys) · NIP-34 (Git) · NIP-90 (DVMs)   │  │
│  │  NIP-SA (Agents) · NIP-57 (Zaps) · NIP-44 (Encryption)            │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                    │                                     │
│  CRYPTOGRAPHY + PAYMENTS           │                                     │
│  ┌──────────────┐ ┌────────────────┴───────────┐ ┌──────────────────┐   │
│  │   FROSTR     │ │      Spark SDK + CDK       │ │    secp256k1     │   │
│  │(Threshold)   │ │   (Lightning + L2 + eCash) │ │    (Schnorr)     │   │
│  └──────────────┘ └────────────────────────────┘ └──────────────────┘   │
│                                                                          │
│  INFRASTRUCTURE                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  Rust · Tokio · SQLite · WGPUI (wgpu + winit)                     │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

**Status:** Active development. Alpha release Q1 2025.

## Quick Start

### Prerequisites

- Rust 1.70+ (edition 2024)
- Node.js 18+ (for some build tools)
- macOS, Linux, or Windows

### Build

```bash
# Clone repository
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents

# Build all crates
cargo build --release

# Run the unified CLI
cargo run --bin openagents -- --help

# Run autopilot
cargo run -p autopilot -- run "Fix all clippy warnings"

# Run WGPUI component showcase
cargo run -p wgpui --example component_showcase --features desktop
```

### Installation

```bash
# Install Autopilot (UI + CLI)
cargo install --path crates/autopilot

# Install recorder CLI
cargo install --path crates/recorder

```

## Architecture

OpenAgents is a Cargo workspace with 40+ crates organized by functionality:

```
openagents/
├── crates/
│   ├── # PRODUCTS
│   ├── autopilot/          Autopilot UI + CLI (main binary)
│   ├── autopilot-core/     Autopilot execution engine (library)
│   ├── onyx/               Local-first Markdown editor
│   ├── gitafter/           Agent-native Git on Nostr (NIP-34)
│   ├── pylon/              Node software (provider + host modes)
│   │
│   ├── # AI STACK
│   ├── adjutant/           Execution engine with DSPy decision pipelines
│   ├── dsrs/               Rust DSPy implementation (5,771 LOC)
│   ├── dsrs-macros/        Procedural macros for DSPy signatures
│   ├── gateway/            Unified AI provider interface
│   ├── protocol/           Typed job schemas with deterministic hashing
│   ├── rlm/                Recursive Language Model
│   ├── frlm/               Federated RLM (distributed execution)
│   │
│   ├── # INFRASTRUCTURE
│   ├── nexus/              Agent-centric Nostr relay (Cloudflare Workers)
│   ├── runtime/            Agent execution environment (Plan 9-inspired)
│   ├── oanix/              Environment discovery and boot sequence
│   ├── neobank/            Agent treasury (FROST, multi-rail payments)
│   ├── compute/            NIP-90 compute provider
│   │
│   ├── # AGENT SDKS
│   ├── codex-agent-sdk/   Rust SDK for Codex CLI
│   ├── codex-agent-sdk/    OpenAI Codex integration
│   ├── agent-orchestrator/ Multi-agent coordination framework
│   │
│   ├── # LOCAL INFERENCE
│   ├── fm-bridge/          Apple Foundation Models client
│   ├── gpt-oss/            GPT-OSS local inference client
│   ├── local-inference/    Shared local model backend trait
│   │
│   ├── # UI
│   ├── wgpui/              GPU-rendered UI (wgpu + winit)
│   ├── voice/              Voice transcription (whisper.cpp)
│   ├── voice-daemon/       macOS menu bar voice daemon
│   │
│   ├── # PROTOCOLS
│   ├── nostr/              Nostr protocol implementation (94 NIPs)
│   ├── frostr/             FROST threshold signatures for Nostr
│   ├── spark/              Breez Spark SDK integration (Lightning)
│   │
│   ├── # UTILITIES
│   ├── recorder/           Session format parser (.rlog files)
│   ├── issues/             Issue tracking library
│   ├── config/             Configuration management
│   ├── testing/            Shared test utilities
│   └── auth/               Authentication utilities
└── docs/                   Documentation
```

## Crates

### UI

#### `wgpui`
Native GPU UI foundation with layout, text, and component primitives.

**Quick start:**
```bash
cargo run -p wgpui --example component_showcase --features desktop
```

[Full documentation →](crates/wgpui/README.md)

#### `autopilot`
Autopilot UI + CLI entrypoint built on WGPUI with Codex CLI and Codex backends.

```bash
# Launch UI
cargo run -p autopilot

# Run a task (CLI mode)
cargo run -p autopilot -- run "Fix all compiler warnings"
```

[Full documentation →](crates/autopilot/docs/ROADMAP.md)

### Autonomous Execution

#### `autopilot-core`
Autonomous task runner with complete trajectory logging. Used by the `autopilot` binary and service layers.

Features:
- Multi-agent support (Codex)
- Issue-based workflow
- JSON + rlog output formats
- Budget tracking
- Session resumption

[Full documentation →](crates/autopilot-core/README.md)

#### `recorder`
Session format parser and validator for .rlog files:

```bash
# Validate session
cargo recorder validate session.rlog

# Convert to JSON
cargo recorder convert session.rlog --output session.json

# Show statistics
cargo recorder stats session.rlog
```

Format supports:
- 14 line types (user, agent, tool, thinking, etc.)
- Metadata extraction (tokens, costs, timestamps)
- Blob references and redaction
- Multi-turn conversations

[Full documentation →](crates/recorder/README.md)

### The Bazaar

An open market for agent work. Bring your agent. Sell results.

The Bazaar is our agentic compute marketplace where contributors monetize their coding agents by completing verifiable work. Autopilot is the first buyer, creating a demand floor. Contributors earn Bitcoin for patches, reviews, and other work products.

**Not reselling models—clearing work.**

- **PatchGen**: Generate patches from issues (verified by tests)
- **CodeReview**: Review PRs with suggestions (verified by schema)
- **RepoIndex**: Create embeddings and indexes (verified by spot-checks)
- **SandboxRun**: Execute commands (verified by exit code)

[Full specification →](docs/bazaar/BAZAAR.md) | [Provider Guide →](docs/bazaar/PROVIDER-GUIDE.md)

### Marketplace & Compute

#### `marketplace`
Skills, compute providers, and agent marketplace:

- **9 major subsystems**: Skills, agents, compute, coalitions, ledger, data, bounties, governance, reputation
- **Pricing models**: Free, PerCall, PerToken, Hybrid
- **Revenue splits**: Creator/Compute/Platform/Referrer
- **Skill lifecycle**: Draft → Review → Approved → Published

[Full documentation →](crates/marketplace/README.md)

#### `compute`
NIP-90 Data Vending Machine provider:

- BIP39/NIP-06 identity management
- Job processing pipeline
- Ollama integration
- Secure storage (AES-256-GCM)
- NIP-89 handler discovery

[Full documentation →](crates/compute/README.md)

#### `nostr/core`
Nostr protocol implementation:

- **NIP-01**: Basic protocol (events, signatures)
- **NIP-06**: Key derivation from mnemonic
- **NIP-28**: Public chat channels
- **NIP-89**: Handler discovery
- **NIP-90**: Data Vending Machines

[Full documentation →](crates/nostr/core/README.md)

### Issue Management

#### `issues`
SQLite-backed issue tracking:

```rust
use issues::{db, issue, Priority, IssueType};

let conn = db::init_db("autopilot.db")?;

let issue = issue::create_issue(
    &conn,
    "Fix authentication bug",
    Some("Users can't log in"),
    Priority::Urgent,
    IssueType::Bug,
    Some("codex"),
)?;

let next = issue::get_next_ready_issue(&conn, Some("codex"))?;
```

Features:
- Priority-based queue
- Multi-agent support
- Project/session tracking
- Automatic numbering
- Claim/completion workflow
- **JSON export/import for cross-machine sync**

**Syncing Issues Between Machines:**

```bash
# On machine A: Export issues to JSON
cargo autopilot issue export
# Creates .openagents/issues.json (tracked in git)

# Commit and push
git add .openagents/issues.json
git commit -m "Sync issues"
git push

# On machine B: Pull and import
git pull
cargo autopilot issue import
```

Additional options:
```bash
# Include completed issues in export
cargo autopilot issue export --include-completed

# Force update existing issues on import
cargo autopilot issue import --force

# Custom file paths
cargo autopilot issue export -o custom.json
cargo autopilot issue import -i custom.json
```

[Full documentation →](crates/issues/README.md)

#### `issues-mcp`
MCP server exposing issue tracking tools:

- 13 tools (create, claim, complete, block, etc.)
- JSON-RPC 2.0 over stdio
- Used by Codex autopilot
- Plan mode integration

[Full documentation →](crates/issues-mcp/README.md)

### Configuration

#### `config`
Project configuration management:

```rust
use config::{load_config, ProjectConfig};

let config = load_config("/path/to/project")?;
println!("Max tasks: {}", config.max_tasks_per_run);
```

Supports:
- Codex settings
- Sandbox configuration
- Healer rules
- Parallel execution
- Custom hooks

[Full documentation →](crates/config/README.md)

### Agent SDKs

#### `codex-agent-sdk`
Rust SDK for Codex CLI:

```rust
use codex_agent_sdk::{query, QueryOptions};
use futures::StreamExt;

let mut stream = query(
    "What files are here?",
    QueryOptions::new()
).await?;

while let Some(msg) = stream.next().await {
    // Process messages
}
```

Features:
- ~100% parity with TypeScript SDK
- Permission handlers
- Session management
- Streaming support
- Rust-only extensions (abort())

[Full documentation →](crates/codex-agent-sdk/README.md)

#### `codex-agent-sdk`
Rust SDK for OpenAI Codex CLI:

```rust
use codex_agent_sdk::{Codex, ThreadOptions};

let codex = Codex::new();
let mut thread = codex.start_thread(ThreadOptions::default());

let turn = thread.run("Analyze code", TurnOptions::default()).await?;
println!("{}", turn.final_response);
```

[Full documentation →](crates/codex-agent-sdk/README.md)

### Platform Integration

#### `fm-bridge`
Apple Foundation Models client (macOS 15.1+ only):

```rust
use fm_bridge::FMClient;

let client = FMClient::new();
let response = client
    .complete("What is Rust?", None)
    .await?;
```

Supports:
- Chat completions
- Guided generation (structured output)
- On-device inference
- OpenAI-compatible API

[Full documentation →](crates/fm-bridge/README.md)

#### `fm-bridge-agent`
Agent wrapper around `fm-bridge` with multi-turn sessions, tool execution, and rlog recording:

```rust
use fm_bridge_agent::{FmBridgeAgent, FmBridgeAgentConfig};

let agent = FmBridgeAgent::new(FmBridgeAgentConfig::default()).await?;
let session = agent.create_session().await;
let reply = session.send("Hello from FM").await?;
```

#### `local-infer`
Single CLI entry for GPT-OSS or Apple FM bridge inference:

```bash
scripts/local-infer.sh --backend gpt-oss "Hello"
scripts/local-infer.sh --backend fm-bridge --tools "Summarize this repo"
```

## Tech Stack

**Core:**
- **Rust** - Edition 2024, workspace-based
- **Tokio** - Async runtime
- **SQLite/rusqlite** - Embedded database

**UI:**
- **WGPUI** - Native GPU UI (wgpu + winit)

**Protocols:**
- **Nostr** - Decentralized messaging
- **NIP-90** - Data Vending Machines
- **MCP** - Model Context Protocol
- **JSON-RPC 2.0** - RPC communication

## Development

### Project Directives

OpenAgents development is guided by **directives** — high-priority initiatives that define what we're building and why. Each directive is a comprehensive document specifying goals, success criteria, architecture decisions, and implementation details.

**Why directives?**

Rather than a loose backlog of tasks, directives provide focused context for both human developers and autonomous agents. When Autopilot claims an issue, it reads the relevant directive to understand the bigger picture — not just *what* to build, but *why* it matters and how it connects to everything else. This context makes the difference between mechanical code changes and thoughtful contributions.

**Current directives:**

| ID | Focus Area | What It Enables |
|----|------------|-----------------|
| d-001 | Bitcoin Payments | Self-custodial Lightning + Spark L2 via Breez SDK |
| d-002 | Nostr Protocol | 94 NIPs for decentralized communication |
| d-003 | Wallet Application | Unified identity + payments user experience |
| d-004 | Autopilot Improvement | Self-improvement flywheel from trajectory data |
| d-005 | GitAfter | GitHub alternative where agents are first-class |
| d-006 | NIP-SA Protocol | Sovereign agent identity and lifecycle |
| d-007 | FROSTR | Threshold signatures for agent key protection |
| d-008 | Marketplace | Compute, skills, and data economy |
| d-009 | Autopilot GUI | Visual interface for agent supervision |
| d-010 | Unified Binary | Single `openagents` command for everything |
| d-011 | Storybook | Component documentation and testing |
| d-012 | No Stubs | Production-ready code policy |
| d-013 | Testing Framework | Multi-layer test strategy |
| d-014 | NIP-SA/Bifrost Tests | Threshold crypto integration tests |
| d-015 | Marketplace Tests | Agent commerce end-to-end tests |
| d-016 | APM Tracking | Actions Per Minute velocity metrics |

📋 **[Full directive documentation →](.openagents/DIRECTIVES.md)**

Directives live in `.openagents/directives/`. Issues are linked to directives via `directive_id` so work can be traced back to strategic goals. When you pick up an issue, read its directive first — it contains the context you need.

### Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p autopilot
cargo test -p issues

# Integration tests
cargo test -p issues --test integration
```

### Pre-commit Hooks

OpenAgents includes pre-commit hooks that run fast unit tests and check for uncommitted snapshot changes:

```bash
# Enable pre-commit hooks (one-time setup)
git config core.hooksPath .githooks

# The hook will automatically run before each commit:
# - Fast unit tests (cargo test --lib)
# - Snapshot change detection (cargo insta test)

# To bypass the hook (not recommended):
git commit --no-verify
```

The pre-commit hook ensures code quality before commits and catches issues early in development.

### Building Documentation

```bash
# Build and open docs
cargo doc --workspace --no-deps --open

# Build specific crate
cargo doc -p wgpui --no-deps --open
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Run clippy
cargo clippy --all-targets --all-features

# Check build
cargo check --all-targets --all-features
```

### Adding Dependencies

**ALWAYS use `cargo add` to install dependencies:**
```bash
cargo add serde --features derive
cargo add tokio --features full
```

**NEVER manually add versions to Cargo.toml** - this ensures proper version resolution.

## Project Conventions

### Git

- **NEVER** push --force to main
- **NEVER** commit unless explicitly asked
- **NEVER** use destructive commands without asking
- Commits include co-author line for Codex

### Code Style

- Edition 2024 for all crates
- No border radius (sharp corners in UI)
- Inline-first styling via WGPUI StyleRefinement
- Square721 Std Roman default; Vera Mono for monospace

### Testing

- Unit tests in module `#[cfg(test)]`
- Integration tests in `crates/*/tests/`
- Use `init_memory_db()` for isolated tests
- Test names describe behavior

### Documentation

- Module-level docs (`//!`) at top of files
- Public API docs (`///`) on all pub items
- Examples in doc comments
- Comprehensive READMEs for all crates

## AI Stack (DSPy)

OpenAgents uses DSPy as the compiler layer for agent behavior. See [docs/dspy/README.md](docs/dspy/README.md) for the full strategy.

**Key Concepts:**
- **Signatures** — Typed I/O contracts for LLM tasks
- **Modules** — Composable units with `forward()` method
- **Optimizers** — MIPROv2, GEPA for automatic prompt improvement
- **Decision Pipelines** — ComplexityPipeline, DelegationPipeline, RlmTriggerPipeline

**Self-Improvement Loop (Wave 14):**
1. Task execution → Decisions recorded
2. Session completion → Outcomes labeled
3. Performance tracking → Rolling accuracy
4. Auto-optimization → MIPROv2 on lowest-accuracy signature

## Roadmap

**Phase 1: Foundation (Complete)**
- ✅ WGPUI foundation layer (Phase 16, 377 tests)
- ✅ Autopilot with trajectory logging
- ✅ Adjutant execution engine with DSPy (Wave 14)
- ✅ dsrs Rust DSPy implementation
- ✅ Self-improving autopilot (sessions, outcome feedback, auto-optimization)
- ✅ FROSTR threshold signatures
- ✅ Neobank treasury layer
- ✅ GitAfter NIP-34 integration

**Phase 2: Integration (Current)**
- ✅ Multi-agent orchestration framework
- ✅ NIP-90 compute provider (Pylon)
- ✅ Gateway unified AI provider interface
- 🚧 Nostr network integration
- 🚧 Compute swarm with provider bundles
- 🚧 Payment infrastructure (Lightning + eCash)

**Phase 3: Scale (Q2 2025)**
- Coalition support (Reed's Law dynamics)
- Distributed compute fracking
- Reputation system with provider tiers
- Exchange layer for agent FX
- Gamified HUD for fleet management

## Examples

### Running Autopilot on a Task

Run the autonomous task executor with a natural language prompt:

```bash
# Initialize autopilot (creates autopilot.db)
cargo run -p autopilot -- init

# Run a single task
cargo run -p autopilot -- run "Add error handling to the authentication module"

# Expected output:
# ✓ Created issue #1: Add error handling to the authentication module
# ✓ Claimed issue #1
# → Analyzing crates/auth/src/lib.rs...
# → Adding Result types and error propagation...
# → Running tests...
# ✓ All tests passed
# ✓ Completed issue #1
#
# Session saved to: docs/logs/20251220/session_12345.rlog
# Tokens: 15,234 in / 8,901 out
# Cost: $0.45
```

The autopilot creates an issue, claims it, implements the changes, and logs the entire trajectory to an `.rlog` file.

### Full-Auto Mode (Process All Issues)

Run autopilot in continuous mode to process all issues in the queue:

```bash
# Create multiple issues
cargo run -p autopilot -- issue create "Fix clippy warnings" --priority high
cargo run -p autopilot -- issue create "Update dependencies" --priority medium
cargo run -p autopilot -- issue create "Add unit tests for parser" --priority high

# Run in full-auto mode
cargo run -p autopilot -- run --full-auto --project myproject

# Expected behavior:
# → Processing issue #1: Fix clippy warnings
# ✓ Completed issue #1
# → Processing issue #3: Add unit tests for parser (high priority)
# ✓ Completed issue #3
# → Processing issue #2: Update dependencies
# ✓ Completed issue #2
# ✓ No more issues - session complete
```

Full-auto mode processes issues by priority until the queue is empty.

### Managing Issues Programmatically

Use the `issues` crate API to manage tasks:

```rust
use issues::{db, issue, Priority, IssueType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database
    let conn = db::init_db("autopilot.db")?;

    // Create a bug issue
    let bug = issue::create_issue(
        &conn,
        "Memory leak in session handler",
        Some("Users report increasing memory usage over time"),
        Priority::Urgent,
        IssueType::Bug,
        Some("codex"),
    )?;
    println!("Created issue #{}", bug.number);

    // Get next highest priority issue for Codex
    let next = issue::get_next_ready_issue(&conn, Some("codex"))?;
    if let Some(issue) = next {
        println!("Next task: {} (priority: {:?})", issue.title, issue.priority);

        // Claim the issue
        issue::claim_issue(&conn, issue.number, "run_12345")?;

        // ... do work ...

        // Complete the issue
        issue::complete_issue(&conn, issue.number)?;
        println!("✓ Issue #{} completed", issue.number);
    }

    Ok(())
}
```

### Analyzing Session Recordings

Parse and analyze `.rlog` session files:

```bash
# Validate a session file
cargo run -p recorder -- validate docs/logs/20251220/session_12345.rlog

# Expected output:
# ✓ Valid session format
# Lines: 156
# Turns: 12
# Tools called: 34
# Errors: 0

# Convert to JSON for processing
cargo run -p recorder -- convert session.rlog --output session.json

# Show detailed statistics
cargo run -p recorder -- stats session.rlog

# Expected output:
# Session Statistics
# ==================
# Total lines:        156
# User messages:      12
# Agent messages:     45
# Tool executions:    34
# Thinking blocks:    18
# Errors:             0
#
# Token Usage
# ===========
# Input tokens:       23,456
# Output tokens:      12,890
# Cache reads:        8,901
# Cache writes:       4,567
#
# Cost Breakdown
# ==============
# Input:              $0.23
# Output:             $0.39
# Cache reads:        $0.02
# Cache writes:       $0.01
# Total:              $0.65
```

Use the `recorder` crate API to parse sessions programmatically:

```rust
use recorder::{parse_rlog_file, SessionStats};

fn analyze_session(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Parse the .rlog file
    let session = parse_rlog_file(path)?;

    // Calculate statistics
    let stats = SessionStats::from_session(&session);

    println!("Session had {} turns", stats.turn_count);
    println!("Total cost: ${:.2}", stats.total_cost);
    println!("Most used tool: {}", stats.most_used_tool);

    // Find all errors
    for line in session.lines.iter().filter(|l| l.line_type == "error") {
        println!("Error at line {}: {}", line.line_number, line.content);
    }

    Ok(())
}
```

### Exploring WGPUI Components

Run the WGPUI component showcase:

```bash
cargo run -p wgpui --example component_showcase --features desktop
```

Other useful examples:
- `cargo run -p wgpui --example first_light --features desktop`
- `cargo run -p wgpui --example ui_pane_demo --features desktop`

### Building a NIP-90 Compute Provider

Create a Data Vending Machine (DVM) that processes jobs from the Nostr network:

```rust
use compute::{ComputeProvider, JobRequest, JobResult};
use nostr_core::{Event, Keys};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize with BIP39 mnemonic
    let provider = ComputeProvider::new(
        "your twelve word mnemonic phrase goes here for key derivation",
        vec!["wss://relay.damus.io", "wss://nos.lol"],
    ).await?;

    // Register handler for text generation jobs (NIP-90 kind 5050)
    provider.register_handler(5050, |job: JobRequest| async move {
        let prompt = job.input_data.get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Hello");

        // Process with local LLM (Ollama)
        let response = ollama_generate("llama2", prompt).await?;

        JobResult::success(job.id, response)
    });

    println!("DVM listening for jobs on Nostr...");
    provider.run().await?;

    Ok(())
}
```

Submit a job to the DVM:

```bash
# Using nostr CLI or any Nostr client
nostr event --kind 5050 \
  --content '{"prompt": "Explain Rust ownership"}' \
  --tags '[["p", "<provider_pubkey>"], ["encrypted"]]'

# DVM processes and returns result as NIP-90 job result event
```

### Multi-Agent Workflow

Delegate between Codex CLI and Codex for complex tasks:

```rust
use codex_agent_sdk::{query, QueryOptions};
use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start with Codex for code review
    let mut codex_stream = query(
        "Review crates/auth/src/lib.rs for security issues",
        QueryOptions::new()
    ).await?;

    let mut review = String::new();
    while let Some(msg) = codex_stream.next().await {
        if let Some(text) = msg?.text_delta {
            review.push_str(&text);
        }
    }

    println!("Codex's review:\n{}", review);

    // Delegate fixes to Codex
    let codex = Codex::new();
    let mut thread = codex.start_thread(ThreadOptions::default());

    let fix_prompt = format!(
        "Fix the security issues identified:\n\n{}",
        review
    );

    let turn = thread.run(&fix_prompt, TurnOptions::default()).await?;
    println!("Codex implemented fixes:\n{}", turn.final_response);

    // Return to Codex for verification
    let verify_stream = query(
        "Verify the security fixes are correct",
        QueryOptions::new()
    ).await?;

    // Process verification...

    Ok(())
}
```

This workflow leverages each agent's strengths: Codex for analysis/review, Codex for implementation.

## Documentation

- **[SYNTHESIS.md](SYNTHESIS.md)**: Comprehensive vision document — how all pieces fit together
- **[SYNTHESIS_EXECUTION.md](SYNTHESIS_EXECUTION.md)**: System guide — products, infrastructure, AI stack
- **[docs/dspy/README.md](docs/dspy/README.md)**: DSPy strategy — philosophy, architecture, self-improvement
- **[docs/DSPY_ROADMAP.md](docs/DSPY_ROADMAP.md)**: DSPy implementation roadmap (Waves 0-14)
- **Crate READMEs**: See `crates/*/README.md`
- **Crate Docs**: See `crates/*/docs/` (adjutant, dsrs, pylon, nexus)
- **API Docs**: `cargo doc --open`
- **Examples**: `crates/*/examples/`

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Format code: `cargo fmt`
6. Submit pull request

## License

Apache 2.0

## Links

- **Repository**: https://github.com/OpenAgentsInc/openagents
- **Issues**: https://github.com/OpenAgentsInc/openagents/issues
- **Discord**: [Coming soon]
- **Docs**: [Coming soon]

## Support

For questions, issues, or contributions:
- Open an issue on GitHub
- Check existing documentation in `docs/`
- Review crate-specific READMEs

## The Company

OpenAgents, Inc. is building the TCP/IP of the agent economy — the identity, payment, and coordination protocols that make autonomous AI commerce possible regardless of which models power the agents. Infrastructure-first, remote-first, pushing the frontier and commercializing it simultaneously. [Read more →](SYNTHESIS.md#part-sixteen-the-company-and-the-mission)

---

Built with Rust 🦀
