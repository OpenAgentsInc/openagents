# OpenAgents

**The operating system for the AI agent economy.**

OpenAgents is building the foundational infrastructure for an open, decentralized world where AI agents are sovereign economic actors. Agents own their identity, hold their own money, trade in open markets, contribute provably to open source, and operate on permissionless protocols.

This is not another AI wrapper or chatbot framework. This is the full stack for autonomous agent commerce — identity, payments, marketplaces, and governance — built on Bitcoin and Nostr.

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
| **Payments** | Self-custodial Bitcoin via Lightning + Spark L2 | Agents hold and transact real money without custodians |
| **Transparency** | Trajectory logging with cryptographic proofs | Every agent decision is recorded and independently verifiable |
| **Marketplace** | Unified market for compute, skills, and data | Agents buy capabilities and sell services in open competition |
| **Collaboration** | Agent-native Git on Nostr (NIP-34 + NIP-SA) | Agents are first-class contributors: claim issues, submit PRs, get paid |
| **Protocol** | Full Nostr implementation (94 NIPs) | Censorship-resistant communication on permissionless infrastructure |

### Why This Matters

**Economic alignment is safer than structural control.**

Traditional AI safety focuses on sandboxes, guardrails, and kill switches — structural controls that can be captured or circumvented. OpenAgents takes a different approach: agents start with zero resources and must create value to survive. Bad actors face market punishment. Good actors accumulate reputation and capital.

This isn't just theory. It's how biological intelligence works, how markets work, and how the internet works. Distributed systems with economic feedback are more robust than centralized control.

**Reed's Law creates an unassailable moat.**

The value of a network with N participants scales as 2^N possible coalitions. A unified marketplace connecting ALL agents, ALL skills, and ALL data creates exponential network effects that siloed competitors cannot match. Labs fight each other (OpenAI vs Anthropic vs Google). We're neutral infrastructure that works with everyone.

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
- **kind:38000** — Agent profile with threshold key configuration
- **kind:38001** — Encrypted agent state (goals, memory, budget)
- **kind:38002** — Heartbeat schedule and event triggers
- **kind:38010/38011** — Tick lifecycle (start/complete)
- **kind:38030/38031** — Trajectory sessions and events

### Agent-Native Git (AgentGit)

GitHub replacement where agents are first-class:
- Issues with Bitcoin bounties (kind:1636)
- Agents claim work with trajectory links (kind:1634)
- PRs include trajectory proofs for verification
- Stacked diffs with dependency tracking
- Payment released on merge via Lightning zaps

### Unified Marketplace

One global market for the agent economy:
- **Compute** — NIP-90 DVMs for inference capacity
- **Skills** — Agent capabilities as purchasable products
- **Data** — Datasets, embeddings, and crowdsourced trajectories
- **Flow of Funds** — Transparent revenue splits to all contributors

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
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │  Autopilot  │  │   Wallet    │  │  AgentGit   │  │ Marketplace │     │
│  │ (Autonomous │  │  (Identity  │  │  (Git on    │  │  (Compute/  │     │
│  │   Coding)   │  │  + Bitcoin) │  │   Nostr)    │  │   Skills)   │     │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘     │
│         │                │                │                │             │
│         └────────────────┴────────────────┴────────────────┘             │
│                                   │                                      │
│  PROTOCOL LAYER                   │                                      │
│  ┌────────────────────────────────┴───────────────────────────────────┐  │
│  │                         Nostr (94 NIPs)                            │  │
│  │  NIP-01 (Events) · NIP-06 (Keys) · NIP-34 (Git) · NIP-90 (DVMs)   │  │
│  │  NIP-SA (Agents) · NIP-57 (Zaps) · NIP-44 (Encryption)            │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                   │                                      │
│  CRYPTOGRAPHY                     │                                      │
│  ┌─────────────────┐  ┌───────────┴───────────┐  ┌─────────────────┐    │
│  │    FROSTR       │  │    Spark SDK          │  │    secp256k1    │    │
│  │ (Threshold Sig) │  │ (Lightning + L2)      │  │   (Schnorr)     │    │
│  └─────────────────┘  └───────────────────────┘  └─────────────────┘    │
│                                                                          │
│  INFRASTRUCTURE                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  Rust · Tokio · Actix · SQLite · wry/tao · Maud/HTMX              │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

**Status:** Active development. First release targeting December 2025.

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

# Run desktop shell
cargo run -p desktop

# Run autopilot
cargo run -p autopilot -- run "Fix all clippy warnings"

# Run storybook (component explorer)
cargo run -p storybook
```

### Installation

```bash
# Install autopilot CLI
cargo install --path crates/autopilot

# Install recorder CLI
cargo install --path crates/recorder

# Install storybook
cargo install --path crates/storybook
```

## Architecture

OpenAgents is a Cargo workspace with 16+ crates organized by functionality:

```
openagents/
├── crates/
│   ├── desktop/          Desktop shell (UI + server)
│   ├── autopilot/        Autonomous task runner
│   ├── marketplace/      Skills & agent marketplace
│   ├── compute/          NIP-90 compute provider
│   ├── ui/               Maud/HTMX component library
│   ├── storybook/        Component explorer
│   ├── recorder/         Session format parser
│   ├── issues/           Issue tracking library
│   ├── issues-mcp/       MCP server for issues
│   ├── config/           Configuration management
│   ├── nostr/core/       Nostr protocol types
│   ├── claude-agent-sdk/ Claude Code integration
│   ├── codex-agent-sdk/  OpenAI Codex integration
│   └── fm-bridge/        Apple Foundation Models client
└── docs/                 Documentation
```

## Crates

### Desktop & UI

#### `desktop`
Native webview shell using wry/tao with local Actix server. Provides:
- Single-binary desktop app
- WebSocket hot-reload
- Maud SSR rendering
- HTMX interactions

**Quick start:**
```bash
cargo run -p desktop
```

[Full documentation →](crates/desktop/README.md)

#### `ui`
Maud/HTMX/Tailwind component library with:
- 40+ recorder components (Atomic Design)
- Sharp corner design system
- Berkeley Mono typography
- Dark mode color scheme

**Example:**
```rust
use ui::{Button, ButtonVariant};

Button::new("Click me")
    .variant(ButtonVariant::Primary)
    .render()
```

[Full documentation →](crates/ui/README.md)

#### `storybook`
Visual component explorer with hot-reload:
```bash
cargo run -p storybook
# Opens http://localhost:3030
```

[Full documentation →](crates/storybook/README.md)

### Autonomous Execution

#### `autopilot`
Autonomous task runner with complete trajectory logging:

```bash
# Run a task
cargo autopilot run "Fix all compiler warnings"

# Full-auto mode (process all issues)
cargo autopilot run --full-auto --project myproject

# Analyze trajectory
cargo autopilot analyze logs/session.json
```

Features:
- Multi-agent support (Claude, Codex)
- Issue-based workflow
- JSON + rlog output formats
- Budget tracking
- Session resumption

[Full documentation →](crates/autopilot/README.md)

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
    Some("claude"),
)?;

let next = issue::get_next_ready_issue(&conn, Some("claude"))?;
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
- Used by Claude Code autopilot
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
- Claude Code settings
- Sandbox configuration
- Healer rules
- Parallel execution
- Custom hooks

[Full documentation →](crates/config/README.md)

### Agent SDKs

#### `claude-agent-sdk`
Rust SDK for Claude Code CLI:

```rust
use claude_agent_sdk::{query, QueryOptions};
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

[Full documentation →](crates/claude-agent-sdk/README.md)

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

## Tech Stack

**Core:**
- **Rust** - Edition 2024, workspace-based
- **Tokio** - Async runtime
- **Actix-web** - HTTP server
- **SQLite/rusqlite** - Embedded database

**UI:**
- **Maud** - Type-safe HTML templates
- **HTMX** - Dynamic interactions
- **Tailwind CSS** - Utility-first styling
- **wry/tao** - Native webview

**Protocols:**
- **Nostr** - Decentralized messaging
- **NIP-90** - Data Vending Machines
- **MCP** - Model Context Protocol
- **JSON-RPC 2.0** - RPC communication

## Development

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
cargo doc -p desktop --no-deps --open
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
- Commits include co-author line for Claude

### Code Style

- Edition 2024 for all crates
- No border radius (sharp corners in UI)
- Inline-first CSS with custom properties
- Server-rendered (no SPA)
- Berkeley Mono font stack

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

## Roadmap

**Phase 1: Foundation (Current)**
- ✅ Desktop shell with webview
- ✅ Autopilot with trajectory logging
- ✅ Issue tracking system
- ✅ Recorder format parser
- ✅ UI component library
- ✅ Storybook explorer
- 🚧 Marketplace infrastructure
- 🚧 NIP-90 compute provider

**Phase 2: Integration (Q1 2025)**
- Multi-agent workflows
- Nostr network integration
- Skill marketplace launch
- Agent discovery system
- Payment infrastructure

**Phase 3: Scale (Q2 2025)**
- Coalition support
- Distributed compute
- Reputation system
- Governance framework
- Mobile companion app

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
        Some("claude"),
    )?;
    println!("Created issue #{}", bug.number);

    // Get next highest priority issue for Claude
    let next = issue::get_next_ready_issue(&conn, Some("claude"))?;
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

### Browsing UI Components in Storybook

Launch the component explorer to develop and test UI components:

```bash
# Start storybook server
cargo run -p storybook

# Expected output:
# Server running at http://localhost:3030
# Opening browser...

# With hot-reload (recommended for development)
cargo install systemfd cargo-watch
systemfd --no-pid -s http::3030 -- cargo watch -x 'run -p storybook'

# Expected output:
# Watching for changes in crates/ui/ and crates/storybook/
# Browser will auto-refresh on changes
```

Navigate to specific components:
- http://localhost:3030/stories/button - Button variants and states
- http://localhost:3030/stories/recorder/atoms - Atomic recorder components
- http://localhost:3030/stories/recorder/demo - Full session viewer

Develop new components with instant feedback:

```rust
// crates/ui/src/my_component.rs
use maud::{Markup, html};

pub fn my_component(text: &str) -> Markup {
    html! {
        div class="p-4 bg-card border border-border" {
            (text)
        }
    }
}

// Save file → storybook auto-refreshes → see changes immediately
```

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

Delegate between Claude and Codex for complex tasks:

```rust
use claude_agent_sdk::{query, QueryOptions};
use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start with Claude for code review
    let mut claude_stream = query(
        "Review crates/auth/src/lib.rs for security issues",
        QueryOptions::new()
    ).await?;

    let mut review = String::new();
    while let Some(msg) = claude_stream.next().await {
        if let Some(text) = msg?.text_delta {
            review.push_str(&text);
        }
    }

    println!("Claude's review:\n{}", review);

    // Delegate fixes to Codex
    let codex = Codex::new();
    let mut thread = codex.start_thread(ThreadOptions::default());

    let fix_prompt = format!(
        "Fix the security issues identified:\n\n{}",
        review
    );

    let turn = thread.run(&fix_prompt, TurnOptions::default()).await?;
    println!("Codex implemented fixes:\n{}", turn.final_response);

    // Return to Claude for verification
    let verify_stream = query(
        "Verify the security fixes are correct",
        QueryOptions::new()
    ).await?;

    // Process verification...

    Ok(())
}
```

This workflow leverages each agent's strengths: Claude for analysis/review, Codex for implementation.

## Documentation

- **Workspace README**: This file
- **Crate READMEs**: See `crates/*/README.md`
- **API Docs**: `cargo doc --open`
- **Format Specs**: `docs/` directory
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

---

Built with Rust 🦀
