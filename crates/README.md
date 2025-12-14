# OpenAgents Crates

This directory contains all Rust crates for the OpenAgents project. All crates use Rust edition 2024.

## Core MVP

### `wgpui/`
**GPU-accelerated UI rendering engine**

Low-level UI framework with wgpu/WebGPU backend:
- Taffy-based flexbox layout
- GPU-accelerated text rendering
- Platform abstraction (web-sys/winit)
- Theme and styling system

### `coder/app/`
**Main application entry point**

Application bootstrap and platform initialization:
- Native/web entry points
- AppState management
- Event processing loop
- View composition

### `coder/shell/`
**Application shell with routing and navigation**

Top-level app chrome and navigation:
- Router (URL ↔ View mapping)
- Navigation controller (back/forward, breadcrumbs)
- ViewRegistry for managing active views
- Window chrome (header, status bar)

### `coder/surfaces_*/`
**IDE surface components**

Specialized UI surfaces:
- `coder/surfaces_chat/` - Chat thread with markdown streaming
- `coder/surfaces_terminal/` - ANSI terminal emulator
- `coder/surfaces_diff/` - Side-by-side/unified diff viewer
- `coder/surfaces_timeline/` - Agent workflow visualization

### `coder/widgets/`
**Widget library**

Reusable UI components:
- Widget trait + AnyWidget
- Div, Text, ScrollView, VirtualList
- TextInput, Button

### `coder/ui_runtime/`
**Reactive UI runtime**

Solid.js-inspired reactivity:
- Signal<T>, Memo<T>, Effect
- Scope management
- Frame scheduler
- Command bus

### `coder/domain/`
**Domain model and events**

Event-sourced domain types:
- DomainEvent (ChatMessageReceived, ToolUseStarted, etc.)
- Projections (ChatView, ThreadSummary)
- Thread and message types

### `coder/protocol/`
**Wire protocol for agent communication**

Protocol types for client-agent messaging:
- Request/Response types
- Message framing
- Serialization

### `claude-agent-sdk/`
**Rust SDK for Claude Code CLI**

Programmatically build AI agents with Claude Code's capabilities:
- Spawn and manage Claude Code CLI processes
- Stream conversation messages
- Handle tool use and results
- Session management

### `config/`
**Project configuration**

Configuration loading, validation, and defaults for OpenAgents applications.

### `tools/`
**File operations, search, shell execution**

Core tools for agents:
- File read/write operations
- Search and pattern matching (grep)
- Shell command execution
- Text manipulation utilities

### `atif/`
**Agent Trajectory Interchange Format**

Standardized JSON format for logging agent interactions:
- Trajectory and step types
- Metrics and observations
- SQLite storage (with `store` feature)

---

## Nostr & Decentralized

### `nostr/`
Nostr protocol implementation organized as submodules:

#### `nostr/core/`
**Core Nostr protocol types (NIP-01, NIP-06, NIP-28, NIP-90)**

- Event, Filter, Subscription types
- Key generation and signing (with `full` feature)
- Minimal types for WASM (with `minimal` feature)

#### `nostr/client/`
**WebSocket client for Nostr relays**

- Connect to multiple relays
- Send events and subscribe to filters
- Handle relay responses

#### `nostr/relay/`
**Nostr relay server implementation**

- NIP-01 compliant relay
- Event storage and filtering
- Subscription management

#### `nostr/chat/`
**Nostr chat state machine**

- NIP-28 channel support
- NIP-90 DVM job integration
- Chat message handling

### `cloudflare/`
**Cloudflare Workers relay and DVM**

Deploy Nostr relay and DVM to Cloudflare Workers:
- Durable Objects for state
- NIP-90 job processing
- Edge-native performance

---

## Agent Infrastructure

### `agents/`
**Agent type system and capabilities**

Foundational types for defining and executing agents:
- AgentId (Nostr-based identity)
- AgentManifest (capabilities declaration)
- AgentExecutor trait
- NIP-90 job protocol support

### `taskmaster/`
**Issue and task tracking**

SQLite-backed issue tracker:
- Issue lifecycle management
- Label filtering and search
- Dependency tracking
- CLI interface

### `oanix/`
**Plan 9-inspired agent operating environment**

Sandboxed execution environment for agents:
- "Everything is a file" namespace
- WASI runtime support
- Capability-based access control
- Browser WASM support

### `unit/`
**MIMO dataflow runtime**

Visual programming / dataflow execution:
- Multi-Input Multi-Output state machines
- Pin-based data flow
- Graph composition

### `fm-bridge/`
**Apple Foundation Models client**

HTTP client for on-device LLM inference (macOS 15.1+):
- OpenAI-compatible API
- Streaming support
- Guided generation

---

## Crate Structure

```
crates/
├── wgpui/                    # GPU-accelerated UI engine
├── coder/
│   ├── app/                  # Main application entry point
│   ├── shell/                # Application shell + routing
│   ├── surfaces_chat/        # Chat thread surface
│   ├── surfaces_terminal/    # Terminal emulator surface
│   ├── surfaces_diff/        # Diff viewer surface
│   ├── surfaces_timeline/    # Timeline surface
│   ├── widgets/              # Widget library
│   ├── ui_runtime/           # Reactive runtime
│   ├── domain/               # Domain model + events
│   └── protocol/             # Wire protocol
├── claude-agent-sdk/         # Claude CLI SDK
├── mechacoder/               # Agent message types
├── config/                   # Configuration
├── tools/                    # File/shell tools
├── atif/                     # Trajectory format + storage
├── nostr/
│   ├── core/                 # Protocol types
│   ├── client/               # WebSocket client
│   ├── relay/                # Relay server
│   └── chat/                 # Chat state machine
├── cloudflare/               # Cloudflare Workers
├── agents/                   # Agent types
├── taskmaster/               # Task tracking
├── oanix/                    # Agent sandbox
├── unit/                     # Dataflow runtime
└── fm-bridge/                # Apple FM client
```
