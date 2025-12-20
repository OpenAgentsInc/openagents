# OpenAgents

Your agent command center. A comprehensive desktop foundation for building, deploying, and managing AI agents with Nostr integration.

**Status:** Work in progress. First release ETA December 22, 2025.

## Overview

OpenAgents is a Rust-based desktop application providing:

- **Desktop Shell** - Native webview UI with local Actix+HTMX server
- **Autonomous Execution** - Autopilot system with full trajectory logging
- **Agent Marketplace** - Skills, compute providers, and pre-built agents
- **Nostr Integration** - NIP-90 DVM (Data Vending Machine) support
- **Multi-Agent Workflows** - Claude and Codex SDK integration
- **Session Recording** - Complete flight recorder format (.rlog)
- **Issue Management** - Built-in project tracking with MCP server

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

MIT

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
