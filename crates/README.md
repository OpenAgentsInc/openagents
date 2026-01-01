# OpenAgents Crates

This directory contains the Rust crates that power OpenAgents. Each section below provides a single-paragraph overview of a crate to explain its role in the workspace. Nested crates use paths like `nostr/core` or `web/worker`.

## acp-adapter
The acp-adapter crate implements the Agent Client Protocol (ACP) adapter for OpenAgents. It wraps agent SDKs, manages JSON-RPC sessions and permissions, and converts ACP traffic to and from rlog trajectories.

## agent
The agent crate defines the core data model for sovereign agents, including configuration, lifecycle state, spawning, and registry persistence. It does not run agents itself; runtimes like Pylon and Nexus build on it.

## agent-orchestrator
The agent-orchestrator crate provides a control plane for multi-model agents, including registry management, lifecycle hooks, and background task orchestration. It integrates with Autopilot and marketplace components to route work across providers.

## auth
The auth crate provides token-based authentication for local services by generating or loading a per-user token, storing it with restricted permissions, and validating it with constant-time comparisons.

## autopilot
The autopilot crate is the core autonomous task runner used by OpenAgents. It executes prompts against codebases via agent SDKs, supports tunnel and container execution modes, and records trajectories for replay.

## autopilot-container
The autopilot-container crate wraps Autopilot in an HTTP service designed for Cloudflare Containers. It exposes start/status APIs and WebSocket streaming for cloud execution.

## autopilot-service
The autopilot-service crate provides the runtime service layer used by Autopilot shells and CLI tooling. It centralizes runtime coordination types, CLI helpers, and session snapshots.

## autopilot-shell
The autopilot-shell crate is the native WGPUI shell for Autopilot, providing the docked HUD layout and wiring to autopilot-service.

## autopilot-wasm
The autopilot-wasm crate exposes WASM bindings for the Autopilot replay viewer, including JSONL parsing, bundle creation, and secret redaction.

## claude-agent-sdk
The claude-agent-sdk crate is a Rust SDK for the Claude Code CLI, providing session management, streaming messages, tool control, and budget/permission configuration.

## claude-mcp
The claude-mcp crate runs a Model Context Protocol (MCP) server that exposes Claude Code via JSON-RPC stdio. It wraps claude-agent-sdk to provide tools for queries and session control.

## codex-agent-sdk
The codex-agent-sdk crate is a Rust SDK for OpenAI's Codex CLI agent, with thread/session management and configurable sandbox and model options.

## compute
The compute crate implements a NIP-90 DVM provider that sells compute by handling Nostr job requests, payments, and result publication.

## config
The config crate loads and validates OpenAgents project configuration from `.openagents/project.json`, providing defaults used across the workspace.

## daytona
The daytona crate is a Rust SDK for the Daytona sandbox API, covering sandbox lifecycle, file operations, and command execution.

## editor
The editor crate provides text editor primitives (buffer, caret, selection, syntax highlighting, and view logic) used by WGPUI surfaces and the web client.

## fm-bridge
The fm-bridge crate is a Rust client for Apple's Foundation Models HTTP bridge with an OpenAI-compatible API surface.

## fm-bridge-agent
The fm-bridge-agent crate wraps fm-bridge in an agent interface that mirrors gpt-oss-agent patterns, including session state and tool execution.

## frostr
The frostr crate is a native Rust implementation of FROSTR threshold Schnorr signing for Nostr identities.

## gitafter
The gitafter crate powers the GitAfter desktop app, a Nostr-native GitHub alternative for browsing repositories, issues, and patches.

## gpt-oss
The gpt-oss crate is a Rust client for the GPT-OSS Responses API, including request/response types and streaming support.

## gpt-oss-agent
The gpt-oss-agent crate wraps gpt-oss in an agent abstraction compatible with ACP tooling, including tool execution and trajectory recording.

## issue-tool
The issue-tool crate is a lightweight CLI for the issues database (create, list, claim, complete, block).

## issues
The issues crate provides the SQLite-backed issue tracking core, including migrations and lifecycle transitions used by Autopilot.

## local-inference
The local-inference crate defines the LocalModelBackend trait and shared request/response types for local inference engines.

## marketplace
The marketplace crate implements the OpenAgents economy for skills, compute providers, data listings, reputation, and payment accounting.

## neobank
The neobank crate defines treasury and payment routing primitives for agent budgets and multi-rail payouts (Lightning, eCash, stable rails).

## nexus
The nexus crate holds the planned cloud runtime for sovereign agents, serving as the design counterpart to Pylon (implementation in progress).

## nostr/core
The nostr/core crate provides core Nostr protocol types, signing, and cryptography shared across OpenAgents.

## nostr/client
The nostr/client crate implements a Nostr relay client for subscribing, publishing, and relay management.

## nostr/relay
The nostr/relay crate is a Nostr relay server implementation with SQLite storage and filtering.

## nostr/tests
The nostr/tests crate contains integration tests that exercise the core, client, and relay crates together.

## opencode-sdk
The opencode-sdk crate is a Rust SDK for OpenCode servers, providing REST and SSE clients for provider-agnostic agent execution.

## orderbook
The orderbook crate is a NIP-69 orderbook viewer for live P2P order flows on Nostr.

## pylon
The pylon crate is the local runtime for sovereign agents, including CLI tooling and provider-mode compute support.

## recorder
The recorder crate parses, validates, and repairs rlog session logs and includes a CLI for stats and formatting.

## relay
The relay crate defines the WebSocket message protocol shared by the browser, worker, and tunnel client in the OpenAgents relay system.

## runtime
The runtime crate provides a pluggable runtime for autonomous agents, including the tick model, storage interfaces, identity, and compute/container abstractions.

## spark
The spark crate integrates Bitcoin Lightning payments (Spark) for OpenAgents wallets and compute flows.

## testing
The testing crate provides shared fixtures and helpers for workspace tests.

## wallet
The wallet crate provides the unified OpenAgents wallet types that tie together Nostr identity and Bitcoin payments.

## web
The web crate is a WGPUI web demo showcasing GPU-accelerated text and Markdown rendering in the browser.

## web/client
The web/client crate is the WGPUI WASM client for the web UI, including app state, views, and relay/tunnel integration.

## web/worker
The web/worker crate is the Cloudflare Worker (Axum) API for the web app, covering auth, billing, wallet, and tunnel endpoints backed by D1/KV.

## web/wallet-worker
The web/wallet-worker crate is the Cloudflare Worker that runs Spark wallet operations and Lightning payment flows.

## web-platform
The web-platform crate is the Actix-based web platform server for GitHub OAuth, Stripe checkout, and managed Autopilot jobs.

## wgpui
The wgpui crate is the GPU-accelerated UI rendering library used by OpenAgents desktop and web clients.
