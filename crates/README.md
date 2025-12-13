# OpenAgents Crates

This directory contains all Rust crates for the OpenAgents project. All crates use Rust edition 2024.

## Core MVP

### `dioxus/`
**Main web application built with Dioxus**

MechaCoder chat UI with Claude integration:
- Dioxus 0.7 fullstack (SSR + hydration)
- Lumen Blocks component library (shadcn/ui-style)
- Server functions for Claude API proxy
- Tailwind CSS styling

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
├── dioxus/              # Main web app (MVP)
├── claude-agent-sdk/    # Claude CLI SDK
├── config/              # Configuration
├── tools/               # File/shell tools
├── atif/                # Trajectory format + storage
├── nostr/
│   ├── core/            # Protocol types
│   ├── client/          # WebSocket client
│   ├── relay/           # Relay server
│   └── chat/            # Chat state machine
├── cloudflare/          # Cloudflare Workers
├── agents/              # Agent types
├── taskmaster/          # Task tracking
├── oanix/               # Agent sandbox
├── unit/                # Dataflow runtime
└── fm-bridge/           # Apple FM client
```
