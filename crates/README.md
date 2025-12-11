# OpenAgents Crates

This directory contains all Rust crates for the OpenAgents project. All crates use Rust edition 2024.

## Core Agent System

### `acp`
**Agent Client Protocol (ACP) connection layer for Claude Code**

Connection layer for Agent Client Protocol:
- ACP protocol implementation
- Claude Code integration
- Agent communication layer

### `acp_thread`
**ACP thread management**

Thread management for ACP connections:
- Thread lifecycle
- Buffer diff handling
- Terminal integration
- Project integration

### `acp_tools`
**ACP tools implementation**

Tool implementations for ACP:
- Tool definitions
- Language integration
- Workspace tools

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

### `gpui_tokio`
**Tokio integration for GPUI**

Async runtime integration for GPUI applications.

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

### `ui_macros`
**Macros for UI components**

Procedural macros for UI component generation.

### `ui_input`
**Input components for UI**

Text input, form controls, and other input widgets.

### `ui_prompt`
**Prompt/dialog components**

Modal dialogs, prompts, and confirmation dialogs.

### `theme`
**Centralized UI theme colors**

Single source of truth for UI colors:
- Consistent theming across all GPUI apps
- Color palette definitions
- Dark/light mode support

### `theme_extension`
**Theme extension system**

Plugin system for custom themes.

### `theme_importer`
**Theme import utilities**

Tools for importing themes from other editors/formats.

### `theme_selector`
**Theme selection UI**

UI component for choosing themes.

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

### `task`
**Task types and utilities**

Core task type definitions and utilities (Zed crate).

### `tasks_ui`
**Task management UI**

GPUI components for task management interface.

### `sessions`
**Claude Code-compatible session storage for conversation replay**

Stores agent conversation sessions:
- Compatible with Claude Code format
- Session replay functionality
- Conversation history

### `session`
**Session management (Zed)**

Session lifecycle and state management (Zed crate).

### `config`
**Project configuration for OpenAgents - loading, validation, defaults**

Configuration management:
- Loading from files
- Validation
- Default values
- Type-safe configuration

### `db`
**Database layer (Zed)**

Database abstraction and SQLite utilities (Zed crate).

### `sqlez`
**SQLite wrapper (Zed)**

Type-safe SQLite wrapper with query builder (Zed crate).

### `sqlez_macros`
**Macros for sqlez**

Procedural macros for SQLite query generation.

### `json_schema_store`
**JSON schema storage**

Storage and validation for JSON schemas.

### `prompt_store`
**Prompt storage**

Storage system for AI prompts and templates.

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

### `http_client_tls`
**TLS configuration for HTTP client**

TLS/SSL configuration utilities for HTTP clients.

### `reqwest_client`
**Reqwest client wrapper**

Alternative reqwest wrapper (Zed crate).

### `aws_http_client`
**AWS HTTP client**

HTTP client with AWS request signing.

### `media`
**Media handling**

Media processing:
- Image handling
- Media encoding/decoding
- Platform-specific media APIs

### `audio`
**Audio processing**

Audio playback, recording, and processing.

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

### `rope`
**Rope data structure (Zed)**

Efficient text rope for editor buffers (Zed crate).

### `text`
**Text utilities (Zed)**

Text manipulation and analysis utilities (Zed crate).

### `multi_buffer`
**Multi-buffer management (Zed)**

Manages multiple editor buffers (Zed crate).

### `paths`
**Path utilities**

Path manipulation and normalization utilities.

### `net`
**Network utilities**

Network abstraction layer and utilities.

### `fs`
**Filesystem utilities (Zed)**

Filesystem operations and abstractions (Zed crate).

### `fsevent`
**Filesystem event monitoring**

File system change notifications (macOS FSEvents, Linux inotify).

### `watch`
**File watching**

File and directory watching utilities.

### `worktree`
**Git worktree utilities (Zed)**

Git worktree management (Zed crate).

### `worktree_benchmarks`
**Worktree performance benchmarks**

Benchmarking for worktree operations.

### `fs_benchmarks`
**Filesystem performance benchmarks**

Benchmarking for filesystem operations.

### `project_benchmarks`
**Project operations benchmarks**

Benchmarking for project-level operations.

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

### `install_cli`
**CLI installation utilities**

Tools for installing and updating CLI binaries.

### `extension_cli`
**Extension CLI commands**

Command-line interface for managing extensions.

### `edit_prediction_cli`
**Edit prediction CLI**

Command-line tools for edit prediction features.

---

## Editor & Language Support

### `editor`
**Text editor core (Zed)**

Core text editor functionality (Zed crate):
- Buffer management
- Cursor handling
- Selection management
- Edit operations

### `vim`
**Vim emulation mode (Zed)**

Vim keybindings and modal editing for Zed editor.

### `vim_mode_setting`
**Vim mode settings**

Configuration for vim mode behavior.

### `language`
**Language support framework (Zed)**

Language server protocol integration and language features (Zed crate).

### `languages`
**Language definitions (Zed)**

Tree-sitter grammars and language configurations (Zed crate).

### `language_extension`
**Language extension system**

Plugin system for language support.

### `language_tools`
**Language tooling utilities**

Utilities for language servers and tooling.

### `language_model`
**Language model integration**

Integration with language models for code completion.

### `language_models`
**Language models abstraction**

Abstraction layer for multiple language model providers.

### `language_onboarding`
**Language onboarding UI**

UI for setting up language support.

### `language_selector`
**Language selection UI**

UI for selecting programming languages.

### `lsp`
**Language Server Protocol client (Zed)**

LSP client implementation (Zed crate).

### `dap`
**Debug Adapter Protocol (Zed)**

DAP client for debugging (Zed crate).

### `dap_adapters`
**DAP adapter implementations**

Adapters for different debugger backends.

### `debug_adapter_extension`
**Debug adapter extension system**

Extension system for debug adapters.

### `debugger_tools`
**Debugger utilities**

Tools and utilities for debugging.

### `debugger_ui`
**Debugger UI components**

GPUI components for debugger interface.

### `tree-sitter-*` (various)
**Tree-sitter grammars**

Parser grammars for various languages (embedded in languages crate).

### `snippet`
**Code snippet system (Zed)**

Snippet expansion and management (Zed crate).

### `snippet_provider`
**Snippet provider abstraction**

Abstraction for snippet sources.

### `snippets_ui`
**Snippets UI**

UI for managing and inserting snippets.

### `outline`
**Code outline (Zed)**

Symbol tree and code outline generation (Zed crate).

### `outline_panel`
**Outline panel UI**

UI panel for displaying code outline.

### `project_symbols`
**Project symbol indexing**

Symbol indexing and search across projects.

### `breadcrumbs`
**Breadcrumb navigation**

Navigation breadcrumbs for code hierarchy.

### `go_to_line`
**Go to line feature**

Navigation to specific line numbers.

### `file_finder`
**File finder (Zed)**

Fast file search and navigation (Zed crate).

### `search`
**Search functionality (Zed)**

Project-wide search (Zed crate).

### `fuzzy`
**Fuzzy matching**

Fuzzy string matching algorithms.

### `picker`
**Picker UI component**

UI component for selecting from lists.

### `command_palette`
**Command palette (Zed)**

Command palette UI and execution (Zed crate).

### `command_palette_hooks`
**Command palette hooks**

Extension points for command palette.

### `tab_switcher`
**Tab switcher**

Quick tab switching UI.

### `repl`
**REPL integration**

Read-eval-print loop for interactive execution.

### `eval`
**Code evaluation**

Code evaluation and execution utilities.

### `eval_utils`
**Evaluation utilities**

Helper utilities for code evaluation.

---

## AI & LLM Integration

### `anthropic`
**Anthropic Claude client**

Client for Anthropic's Claude API.

### `open_ai`
**OpenAI client**

Client for OpenAI API (GPT-4, etc.).

### `google_ai`
**Google AI client**

Client for Google's Gemini API.

### `deepseek`
**DeepSeek client**

Client for DeepSeek AI API.

### `mistral`
**Mistral client**

Client for Mistral AI API.

### `x_ai`
**xAI (Grok) client**

Client for xAI's Grok API.

### `codestral`
**Codestral client**

Client for Codestral code model.

### `ollama`
**Ollama client**

Client for local Ollama models.

### `lmstudio`
**LM Studio client**

Client for LM Studio local models.

### `open_router`
**OpenRouter client**

Client for OpenRouter aggregation service.

### `cloud_llm_client`
**Cloud LLM client**

Client for cloud-hosted LLM services.

### `cloud_zeta2_prompt`
**Zeta2 prompt templates**

Prompt templates for Zeta2 model.

### `bedrock`
**AWS Bedrock client**

Client for AWS Bedrock LLM service.

### `copilot`
**GitHub Copilot integration**

GitHub Copilot code completion integration:
- Copilot API client
- Code suggestion handling
- Node.js runtime integration
- LSP integration

### `supermaven`
**Supermaven integration**

Integration with Supermaven code completion.

### `supermaven_api`
**Supermaven API client**

Client for Supermaven API.

### `web_search`
**Web search integration**

Web search capabilities for AI agents.

### `web_search_providers`
**Web search providers**

Abstraction for different search engines.

### `assistant_slash_command`
**Assistant slash commands**

Slash command system for AI assistant.

### `assistant_slash_commands`
**Assistant slash commands collection**

Collection of slash command implementations.

### `assistant_text_thread`
**Assistant text thread**

Thread management for assistant conversations.

### `ai_onboarding`
**AI onboarding UI**

UI for setting up AI features.

### `action_log`
**Action logging**

Logging system for user actions and events.

### `agent_settings`
**Agent settings**

Configuration and settings for agent features.

### `agent_ui`
**Agent UI components**

UI components for agent features and interactions.

### `agent_servers`
**Agent server management**

Server-side components for agent execution.

---

## Collaboration & Remote

### `collab`
**Collaboration core (Zed)**

Real-time collaboration engine (Zed crate).

### `collab_ui`
**Collaboration UI**

UI components for collaboration features.

### `remote`
**Remote development (Zed)**

Remote workspace and server management (Zed crate).

### `remote_server`
**Remote server management**

Server-side components for remote development.

### `client`
**Client utilities**

Client-side networking and RPC utilities.

### `rpc`
**RPC framework (Zed)**

Remote procedure call framework (Zed crate).

### `proto`
**Shared protocol for communication between Zed app and zed.dev server**

Protocol definitions using Protocol Buffers:
- Client-server communication protocol
- Type definitions
- Serialization/deserialization

### `channel`
**Channel communication**

Channel-based communication primitives.

### `call`
**Call management**

Voice/video call integration.

### `livekit_api`
**LiveKit API client**

Client for LiveKit real-time communication.

### `livekit_client`
**LiveKit client SDK**

SDK for LiveKit WebRTC integration.

### `harbor`
**Harbor collaboration**

Harbor collaboration protocol implementation.

### `nc`
**Network communication**

Low-level network communication utilities.

---

## Project & Workspace

### `project`
**Project management (Zed)**

Project loading, indexing, and management (Zed crate).

### `workspace`
**Workspace management (Zed)**

Workspace state and window management (Zed crate).

### `project_panel`
**Project panel UI**

UI panel for project file tree.

### `explorer_command_injector`
**Explorer command injection**

Command injection for file explorer.

### `recent_projects`
**Recent projects**

Recent project tracking and UI.

### `component`
**Component system**

Component-based architecture utilities.

### `buffer_diff`
**Buffer diff visualization**

Visual diff for editor buffers.

### `streaming_diff`
**Streaming diff algorithm**

Efficient diff algorithm for large files.

---

## UI Components & Panels

### `panel`
**Panel system (Zed)**

Panel management and layout (Zed crate).

### `terminal`
**Terminal emulator (Zed)**

Terminal emulation and PTY handling (Zed crate).

### `terminal_view`
**Terminal view UI**

UI component for terminal display.

### `markdown`
**Markdown rendering (Zed)**

Markdown parser and renderer (Zed crate).

### `markdown_preview`
**Markdown preview panel**

UI for previewing markdown files.

### `html_to_markdown`
**HTML to Markdown conversion**

Conversion utilities for HTML content.

### `rich_text`
**Rich text rendering**

Rich text formatting and display.

### `image_viewer`
**Image viewer**

Image display and viewing components.

### `svg_preview`
**SVG preview**

SVG rendering and preview.

### `file_icons`
**File icon system**

Icon mapping for file types.

### `icons`
**Icon library**

Icon assets and rendering.

### `assets`
**Asset management**

Asset loading and caching.

### `activity_indicator`
**Activity indicator**

Loading and progress indicators.

### `menu`
**Menu system**

Context menus and menu bars.

### `title_bar`
**Title bar customization**

Custom title bar rendering.

### `notifications`
**Notification system**

System notifications and alerts.

### `feedback`
**Feedback UI**

User feedback collection UI.

### `inspector_ui`
**Inspector UI**

Development inspector and debugging UI.

### `miniprofiler_ui`
**Mini profiler UI**

Performance profiling UI.

### `onboarding`
**Onboarding flow**

User onboarding and tutorials.

### `settings`
**Settings system (Zed)**

Application settings management (Zed crate).

### `settings_json`
**Settings JSON schema**

JSON schema for settings.

### `settings_macros`
**Settings macros**

Procedural macros for settings.

### `settings_ui`
**Settings UI**

UI for application settings.

### `settings_profile_selector`
**Settings profile selector**

UI for selecting settings profiles.

### `release_channel`
**Release channel management**

Beta, stable, and nightly channel handling.

### `feature_flags`
**Feature flags**

Feature flag system for gradual rollouts.

---

## Extensions & Plugins

### `extension`
**Extension system (Zed)**

Extension loading and management (Zed crate).

### `extension_api`
**Extension API**

API surface for extensions.

### `extension_host`
**Extension host**

Runtime for executing extensions.

### `extensions_ui`
**Extensions UI**

UI for managing extensions.

### `theme_extension`
**Theme extension system**

Plugin system for custom themes.

---

## Development Tools

### `git`
**Git integration (Zed)**

Git operations and status (Zed crate).

### `git_ui`
**Git UI components**

UI components for Git operations.

### `git_hosting_providers`
**Git hosting provider integration**

Integration with GitHub, GitLab, etc.

### `diagnostics`
**Diagnostics display**

Error and warning display in editor.

### `keymap_editor`
**Keymap editor**

UI for editing keyboard shortcuts.

### `line_ending_selector`
**Line ending selector**

UI for choosing line endings (LF/CRLF).

### `toolchain_selector`
**Toolchain selector**

UI for selecting Rust/other toolchains.

### `prettier`
**Prettier integration**

Code formatting via Prettier.

### `edit_prediction`
**Edit prediction**

AI-powered edit prediction and completion.

### `edit_prediction_context`
**Edit prediction context**

Context gathering for edit prediction.

### `edit_prediction_types`
**Edit prediction types**

Type definitions for edit prediction.

### `edit_prediction_ui`
**Edit prediction UI**

UI components for edit prediction.

### `denoise`
**Code denoising**

AI-powered code cleanup and simplification.

### `rules_library`
**Rules library**

Code rules and linting library:
- Rule definitions
- Language-specific rules
- Rule execution engine

### `schema_generator`
**Schema generator**

JSON schema generation utilities:
- Schema generation from types
- Validation schemas
- Type-to-schema conversion

### `scheduler`
**Task scheduler (Zed)**

Task scheduling and execution system:
- Async task scheduling
- Priority queues
- Task lifecycle management

---

## System Integration

### `auto_update`
**Auto-update system**

Automatic application updates.

### `auto_update_helper`
**Auto-update helper**

Helper process for applying updates.

### `auto_update_ui`
**Auto-update UI**

UI for update notifications and installation.

### `system_specs`
**System specifications**

Hardware and OS information gathering.

### `credentials_provider`
**Credentials provider**

Secure credential storage and retrieval.

### `askpass`
**Askpass utility**

Password prompt for Git and other tools.

### `telemetry`
**Telemetry system**

Usage analytics and telemetry.

### `telemetry_events`
**Telemetry event definitions**

Event types for telemetry.

### `crashes`
**Crash reporting**

Crash detection and reporting.

### `migrator`
**Data migration**

Database and data migration utilities.

### `journal`
**Journal system**

Event journaling and logging.

### `zlog`
**Zed logging**

Structured logging system.

### `zlog_settings`
**Zed log settings**

Configuration for logging.

### `ztracing`
**Zed tracing**

Distributed tracing integration.

### `ztracing_macro`
**Tracing macros**

Procedural macros for tracing.

### `time_format`
**Time formatting**

Time and date formatting utilities.

### `clock`
**Clock utilities**

Time and clock management.

### `vercel`
**Vercel integration**

Integration with Vercel deployment platform:
- Deployment management
- Project configuration
- API integration

### `story`
**Story system**

Story-based UI component system:
- Component stories
- Interactive examples
- Documentation

---

## Language-Specific Tools

### `node_runtime`
**Node.js runtime integration**

Node.js execution environment.

### `context_server`
**Context server**

Language server for context gathering.

---

## Testing & Benchmarks

### `fs_benchmarks`
**Filesystem performance benchmarks**

Benchmarking for filesystem operations.

### `worktree_benchmarks`
**Worktree performance benchmarks**

Benchmarking for worktree operations.

### `project_benchmarks`
**Project operations benchmarks**

Benchmarking for project-level operations.

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

### `mechacoder`
**MechaCoder agent implementation**

Autonomous coding agent (may be merged with `agent` crate).

---

## Zed-Specific Crates

Many crates in this directory are from the Zed editor codebase. These provide:
- Editor functionality (editor, vim, language support)
- UI framework (gpui, ui components)
- Project management (project, workspace)
- Collaboration (collab, remote)
- Extensions system
- And many other editor features

These crates are included to support the Commander desktop app and may be customized for OpenAgents use cases.

### `zed`
**Zed editor main application**

The main Zed code editor application:
- Entry point for Zed editor
- Application lifecycle
- Window management
- Feature integration

### `zed_actions`
**Zed actions system**

Action definitions and registration for Zed:
- Action types
- Action schemas
- Action execution

### `zed_env_vars`
**Zed environment variables**

Environment variable management for Zed.

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
