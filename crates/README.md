# OpenAgents Crates

This directory contains all Rust crates for the OpenAgents project. All crates use Rust edition 2024.

## Core Agent System

### `agent`
**Autonomous coding agent with orchestrator/subagent architecture**

Implements the MechaCoder autonomous coding agent with:
- Orchestrator/subagent decomposition
- Git worktree isolation for parallel execution
- Claude Code MCP integration
- Golden Loop execution (decompose → implement → verify → commit)
- Checkpoint and recovery system
- Tool execution and logging

**Key files:**
- `agent_loop.rs` - Main agent execution loop
- `orchestrator.rs` - Task orchestration
- `subagent.rs` - Subagent implementation
- `worktree_runner.rs` - Git worktree isolation

### `orchestrator`
**Golden Loop agent orchestrator - session management, task selection, verification**

Manages the high-level agent execution flow:
- Session lifecycle management
- Task selection and prioritization
- Verification of completed work
- Integration with tasks, tools, and LLM layers

### `parallel`
**Parallel agent execution with git worktree isolation**

Enables running multiple agents in parallel:
- Git worktree creation for isolation
- Task distribution across worktrees
- Coordination between parallel agents

### `tools`
**Core tools for OpenAgents - file operations, search, shell execution**

Provides fundamental tooling for agents:
- File read/write operations
- Search and pattern matching
- Shell command execution
- Text manipulation utilities

### `llm`
**Multi-provider LLM abstraction layer**

Unified interface for multiple LLM providers:
- Anthropic (Claude)
- OpenAI (GPT-4)
- Google (Gemini)
- Ollama (local)
- OpenRouter (aggregator)
- FM-Bridge (Apple Foundation Model)

### `sandbox`
**Container sandbox execution for OpenAgents**

Provides isolated execution environments:
- Docker-based sandboxing
- Process isolation
- Resource limits
- Used for running untrusted code safely

---

## Terminal-Bench & Training

### `hillclimber`
**MAP-based overnight optimization system for Terminal-Bench**

Implements the HillClimber architecture for optimizing agent performance:
- Meta-learning through test generation evolution
- Configuration space exploration
- Overnight optimization runs
- SQLite storage for run history
- Docker integration for test execution

**Key concept:** Proves "architecture beats model size" by optimizing small models through better test generation and iteration.

### `testgen`
**Test generation and evolution system for Terminal-Bench**

Generates and evolves test cases for agent training:
- Test case generation from task descriptions
- Evolution of test suites over time
- SQLite storage for test history
- Integration with FM-Bridge for generation
- Parameter discovery and refinement

### `gym`
**Training & benchmarking UI (GPUI-based)**

Desktop UI for:
- Running Terminal-Bench tasks
- Viewing hillclimber optimization runs
- Monitoring test generation
- Visualizing agent performance metrics

---

## Foundation Models

### `fm-bridge`
**Rust client for Apple Foundation Models API**

HTTP client for Apple's on-device LLM inference:
- OpenAI-compatible `/v1/chat/completions` endpoint
- Streaming support (when Swift bridge implements SSE)
- Guided generation with pre-defined schemas
- Health checks and model listing
- CLI tool (`fm` command)

**See:** [crates/fm-bridge/README.md](fm-bridge/README.md) for detailed documentation.

---

## UI Framework

### `gpui`
**Zed's GPU-accelerated UI framework**

High-performance UI framework from Zed Industries:
- Hybrid immediate/retained mode rendering
- GPU-accelerated (Metal on macOS)
- Entity-based state management
- Declarative view system
- Low-level element API for custom rendering

**Note:** This is a fork/copy of Zed's GPUI framework, not a dependency.

### `gpui_macros`
**Macros for GPUI**

Procedural macros for GPUI:
- `#[derive(Render)]` - View rendering
- `#[derive(IntoElement)]` - Element conversion
- `AppContext`, `VisualContext` - Context derivation
- Action registration macros

### `hud`
**GPUI visualization layer for Unit dataflow graphs**

Visualizes Unit runtime dataflow:
- Graph rendering of MIMO finite state machines
- Interactive node visualization
- Connection visualization
- Integration with Unit runtime

### `ui`
**Shared UI components**

Reusable GPUI components:
- Common widgets and elements
- Shared styling patterns
- Cross-crate UI primitives

### `theme`
**Centralized UI theme colors**

Single source of truth for UI colors:
- Consistent theming across all GPUI apps
- Color palette definitions
- Dark/light mode support

### `storybook`
**Visual storybook for unit and hud components**

Development tool for viewing UI components:
- Interactive component browser
- Component documentation
- Visual testing

### `commander`
**Desktop app UI for OpenAgents**

Main desktop application built with GPUI:
- Multi-screen navigation (Gym, Wallet, Compute, Marketplace, Chat, Vibe)
- Real-time agent monitoring
- Bitcoin wallet integration
- Swarm compute management
- ATIF trajectory visualization

**See:** [crates/commander/README.md](commander/README.md) for details.

---

## Data & Storage

### `atif`
**Agent Trajectory Interchange Format (ATIF) - Rust implementation**

Standardized format for agent action logs:
- Version 1.4.0 specification
- Trajectory, step, tool call types
- Observation and result types
- Serialization/deserialization

**Purpose:** Enables replay, debugging, visualization, and sharing of agent runs.

### `atif-store`
**SQLite storage layer for ATIF trajectories**

Persistent storage for agent trajectories:
- SQLite database backend
- Trajectory insertion and querying
- Session management
- Integration with ATIF types

### `tasks`
**Task system for OpenAgents - CRUD, priority queue, dependency resolution**

Task management system:
- Task CRUD operations
- Priority queue for task selection
- Dependency resolution
- SQLite storage
- Task metadata and status tracking

### `sessions`
**Claude Code-compatible session storage for conversation replay**

Stores agent conversation sessions:
- Compatible with Claude Code format
- Session replay functionality
- Conversation history

### `config`
**Project configuration for OpenAgents - loading, validation, defaults**

Configuration management:
- Loading from files
- Validation
- Default values
- Type-safe configuration

---

## Nostr & Decentralized

### `nostr`
**Nostr protocol implementation for OpenAgents**

Core Nostr functionality:
- Keypair generation and management
- Event signing and verification
- BIP39 mnemonic support
- Bech32 encoding (nsec/npub)

### `nostr-chat`
**Nostr chat state machine for OpenAgents (NIP-28 channels + NIP-90 DVM)**

Chat functionality over Nostr:
- NIP-28 channel support
- NIP-90 DVM (Data Validation Machine) integration
- State machine for chat sessions
- Relay communication

### `nostr-relay`
**Nostr relay WebSocket client for OpenAgents**

WebSocket client for Nostr relays:
- Connection management
- Event subscription
- Message sending/receiving
- Reconnection handling

### `marketplace`
**Marketplace UI for OpenAgents - Agent Store, Compute Market, Services Market**

GPUI-based marketplace interface:
- Browse and publish agents
- Compute trading
- Services marketplace
- NIP-90 DVM job submission

---

## Infrastructure

### `unit`
**Unit runtime - MIMO finite state machines for dataflow programming**

Dataflow programming runtime:
- Multiple Input, Multiple Output (MIMO) state machines
- Dataflow graph execution
- Node-based computation model
- Used by HUD for visualization

### `collections`
**Standard collection type re-exports used by Zed and GPUI**

Collection type aliases:
- `HashMap`, `HashSet` using `FxHashMap`/`FxHashSet` (fast hashing)
- `IndexMap`, `IndexSet` with fast hashing
- Re-exports from Zed/GPUI codebase

### `util`
**Utilities**

General-purpose utilities:
- Common helper functions
- Shared types and traits
- Cross-crate utilities

### `util_macros`
**Utility macros**

Procedural macros for utilities:
- Build-time code generation
- Helper macros

### `http_client`
**HTTP client (reqwest wrapper)**

HTTP client abstraction:
- Wrapper around reqwest
- Used by GPUI and other crates
- Consistent HTTP interface

### `media`
**Media handling**

Media processing:
- Image handling
- Media encoding/decoding
- Platform-specific media APIs

### `perf`
**Performance utilities**

Performance measurement and profiling:
- Timing utilities
- Performance counters
- Profiling helpers

### `refineable`
**Refineable trait**

Trait for incremental updates:
- Used by GPUI for efficient updates
- Incremental state refinement
- Derive macro support

### `sum_tree`
**Sum tree data structure**

Efficient tree data structure:
- Used by GPUI for text editing
- O(log n) operations
- Sum aggregation over ranges

---

## Learning & Research

### `archivist`
**Trajectory analysis and pattern extraction for the learning system**

Analyzes agent trajectories:
- Pattern extraction from ATIF data
- Learning insights
- Trajectory mining

### `guardrails`
**Safety constraints and resource limits for the learning system**

Safety mechanisms:
- Resource limits
- Safety constraints
- Prevents runaway agents

### `healer`
**Self-healing subagent for detecting and recovering from agent failures**

Automatic failure recovery:
- Detects agent failures
- Attempts recovery
- Self-healing mechanisms

### `reflexion`
**Self-critique and reflection system for MechaCoder**

Agent self-improvement:
- Self-critique of agent actions
- Reflection on failures
- Learning from mistakes

---

## CLI & Tools

### `cli`
**Command-line interface for OpenAgents**

CLI tool for OpenAgents:
- `mechacoder` command - Run autonomous agents
- `session` command - Manage sessions
- `tasks` command - Task management
- Colored output and tables

---

## Future/Planned

### `oanix`
**OpenAgents Agent Operating Environment (Plan 9-style)**

Agent OS inspired by Plan 9:
- Everything is a file/service
- Per-process namespaces
- Capability-based security
- WASI-first execution
- Virtual filesystem abstraction

**Status:** Planned - See [crates/oanix/README.md](oanix/README.md) for architecture.

### `vibe`
**Agentic development environment for building products**

Agentic IDE for "vibe coding":
- Scaffold apps via prompts
- AI-assisted refactoring
- ATIF trajectory viewing
- Full-stack development (React + Rust backend)

**Status:** Planned - See [crates/vibe/README.md](vibe/README.md) for architecture.

---

## Crate Dependencies

### Core Dependencies
Most crates depend on:
- `serde` / `serde_json` - Serialization
- `tokio` - Async runtime
- `thiserror` / `anyhow` - Error handling
- `uuid` - ID generation
- `chrono` - Time handling

### UI Dependencies
GPUI-based crates depend on:
- `gpui` - UI framework
- `theme` - Theming
- `ui` - Shared components

### Agent Dependencies
Agent crates depend on:
- `agent` - Core agent system
- `llm` - LLM abstraction
- `tools` - Tool execution
- `atif` - Trajectory format

---

## Building

All crates are part of a Cargo workspace. Build from the root:

```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p agent

# Run tests
cargo test

# Run specific crate tests
cargo test -p agent
```

## Edition

**All crates use Rust edition 2024.** This is specified in the workspace `Cargo.toml` and should not be changed without discussion.

