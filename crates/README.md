# OpenAgents Crates

This directory contains the Rust crates that power OpenAgents. Each section below provides a single-paragraph overview of a crate to explain its role in the workspace.

## acp-adapter
The acp-adapter crate standardizes how OpenAgents talks to external coding agents by implementing the Agent Client Protocol (ACP) over JSON-RPC 2.0. It wraps SDKs like claude-agent-sdk and codex-agent-sdk to launch subprocesses, manage sessions, and translate ACP updates into OpenAgents rlog and trajectory streams for recording, replay, and UI consumption. Permission handling and conversion utilities live here so higher-level tools can swap agents without changing protocol wiring.

## agent-orchestrator
The agent-orchestrator crate is the control plane for multi-model agents in OpenAgents. It provides agent registry and configuration, lifecycle hooks, background task management, and routing across providers while integrating with directives, autopilot, and marketplace economics. Use it when you need coordinated, policy-aware execution across multiple agent personas or backends.

## auth
The auth crate provides lightweight authentication for localhost services by generating, persisting, and validating a random token. It stores the token in a secure per-user data directory, applies strict file permissions on Unix, and uses constant-time comparisons to avoid timing attacks. This crate is meant for protecting local HTTP or IPC endpoints without external identity providers.

## autopilot
The autopilot crate implements the autonomous task runner that executes prompts with agent SDKs and records full trajectories for audit, replay, and analytics. It includes CLI commands for run, resume, analyze, and compare workflows, integrates with the issues database for queue management, and supports full-auto processing. This is the core engine behind OpenAgents automated issue execution.

## autopilot-gui
The autopilot-gui crate provides the native desktop interface for Autopilot using WGPUI and a winit event loop. It renders dashboards, parallel task views, context panes, and live log streams while driving a background backend that watches session artifacts. Use it for a visual, GPU-rendered view of Autopilot activity.

## claude-agent-sdk
The claude-agent-sdk crate is a Rust interface to the Claude Code CLI that lets OpenAgents spawn sessions, stream messages, and control permissions and budgets programmatically. It mirrors the official SDK surface while adding Rust-specific ergonomics and process control helpers. This crate is the foundation for integrating Claude Code into OpenAgents workflows.

## claude-mcp
The claude-mcp crate exposes Claude Code capabilities as Model Context Protocol tools, wrapping claude-agent-sdk behind a JSON-RPC stdio server. It provides tools to run queries, stream responses, resume sessions, and configure permission modes and budgets from any MCP-aware client. This enables other agents or UIs to delegate work to Claude through a stable tool interface.

## codex-agent-sdk
The codex-agent-sdk crate provides a Rust SDK for OpenAI's Codex CLI agent, including thread and session management, streaming events, and configurable sandbox and model options. It is used when OpenAgents needs to run Codex as a subprocess while still controlling working directories, approvals, and output schemas. The API mirrors Codex CLI semantics in a Rust-friendly wrapper.

## compute
The compute crate implements a NIP-90 Data Vending Machine provider that sells compute by processing Nostr job requests and returning results over relays. It wires together identity management, relay subscriptions, job bidding and handling, and local inference execution while tracking payments. This crate is the core of the OpenAgents compute provider workflow.

## config
The config crate manages OpenAgents project configuration stored in .openagents/project.json, including defaults, validation, and schema evolution. It centralizes settings for sandboxes, Claude Code integration, safety constraints, and runtime limits so all tools read a consistent configuration source. Use it to load or persist project-level settings from any crate.

## fm-bridge
The fm-bridge crate is a Rust client for the Apple Foundation Models HTTP bridge that provides OpenAI-compatible chat completions and model listing. It offers typed requests, async APIs, and a CLI for quick testing while abstracting the underlying Swift bridge. This crate enables local on-device inference on macOS systems that expose the bridge.

## frostr
The frostr crate is a native Rust implementation of the FROSTR threshold Schnorr signing protocol for Nostr. It supports key generation, share management, and signing flows that allow k-of-n participants to sign without reconstructing the private key. This provides the cryptographic foundation for sovereign agent identities and distributed signing.

## gitafter
The gitafter crate is a Nostr-native GitHub alternative that renders a desktop app for browsing and collaborating on NIP-34 git events. It ties together local git operations, a local web UI, and relay subscriptions so agents and humans can view repos, issues, patches, and trajectories in an agent-first workflow. This is the main application crate behind the GitAfter product.

## gpt-oss
The gpt-oss crate is a Rust client for the GPT-OSS Responses API used to talk to open-weight models via HTTP. It provides request and response types, streaming chunk handling, and a backend abstraction for invoking local or self-hosted inference endpoints. This crate is the low-level client used by higher-level agent wrappers.

## gpt-oss-agent
The gpt-oss-agent crate wraps gpt-oss in an agent-level abstraction compatible with the ACP adapter, including tool execution and trajectory recording. It implements session management, tool handling, and rlog logging so GPT-OSS-backed agents behave like other OpenAgents agents. Use it when you need GPT-OSS to participate in multi-agent workflows.

## issue-tool
The issue-tool crate is a lightweight CLI for interacting with the issues SQLite database. It supports create, list, claim, complete, and block operations, making it easy to manage the queue from a terminal without MCP integration. This is the simplest interface for manual issue operations.

## issues
The issues crate provides the SQLite-backed issue tracking core used by Autopilot, including schema migrations, issue lifecycle transitions, and project and session tracking. It exposes a type-safe API for creating, claiming, blocking, and completing issues, plus priority ordering and agent assignment. This is the authoritative source of truth for work queue state.

## issues-mcp
The issues-mcp crate is an MCP server that exposes the issues database as tools over JSON-RPC stdio. It provides endpoints for creating, listing, claiming, completing, and blocking issues, plus plan-mode utilities for orchestrators. This makes the issues queue available to any MCP-aware agent or UI.

## local-inference
The local-inference crate defines shared request and response types and the LocalModelBackend trait that standardizes how OpenAgents talks to local model backends. It supports full and streaming completions, model discovery, readiness checks, and shutdown semantics. This crate allows fm-bridge, gpt-oss, and future backends to share a common interface.

## marketplace
The marketplace crate implements the agent economy layer, including skill listings, compute providers, agent profiles, reputations, and payment accounting. It models how agents buy and sell services, form coalitions, and route NIP-90 jobs while tracking credit flows and governance. This is the economic backbone for OpenAgents marketplace features.

## nostr/core
The nostr core crate implements fundamental Nostr protocol types and cryptography, including events, filters, signing, and key derivation with optional full-crypto features. It provides the shared data model used by clients, relays, wallets, and compute providers. This crate is the protocol foundation for all Nostr integration in OpenAgents.

## nostr/client
The nostr client crate implements a WebSocket client for connecting to relays, subscribing to filters, and publishing events. It layers async networking, relay management, and optional local persistence on top of the core Nostr types. This crate powers OpenAgents components that need to talk to the Nostr network.

## nostr/relay
The nostr relay crate is a Nostr relay server implementation with SQLite-backed storage, filtering, and rate limiting. It exposes websocket endpoints for publishing and subscribing to events while enforcing protocol rules. This crate enables running a self-hosted relay for development or production.

## nostr/tests
The nostr integration-tests crate provides end-to-end tests that exercise the core, client, and relay crates together. It spins up relay instances, sends events, and validates protocol behavior across the full stack. This crate is used to verify that the Nostr subsystem works as a cohesive system.

## opencode-sdk
The opencode-sdk crate is a Rust SDK generated from OpenAPI specs for the OpenCode server, offering typed REST and SSE clients. It handles session management, event streaming, file operations, and provider discovery for OpenCode deployments. Use it to integrate OpenAgents with OpenCode-compatible agent servers.

## recorder
The recorder crate parses, validates, and repairs OpenAgents rlog files, the line-based flight recorder format for agent sessions. It includes both a library and CLI for stats, parsing, and formatting, making trajectories auditable and streamable. This crate underpins logging and replay across agent runtimes.

## spark
The spark crate provides Spark Bitcoin payment integration for OpenAgents, centered on deriving Spark-compatible keys from a BIP39 mnemonic and structuring wallet interfaces. It is designed to plug into the Breez Spark SDK for Lightning and Layer 2 operations while sharing identity with Nostr components. This crate is the payment bridge for future wallet and marketplace flows.

## testing
The testing crate centralizes shared test utilities such as fixtures, mock relays, and helper types used across the workspace. It reduces duplication by providing common patterns for integration and unit tests. Use it when a crate needs standardized test scaffolding.

## wallet
The wallet crate is the unified OpenAgents wallet that ties together Nostr identity and Bitcoin payments from a single mnemonic. It provides core wallet types, storage helpers, and CLI surfaces for identity and payment operations. This crate is the user-facing wallet layer that other systems build on.

## wgpui
The wgpui crate is the GPU-accelerated UI rendering library that powers OpenAgents native desktop interfaces. It combines a component system, layout engine, text rendering, and a wgpu-based renderer to deliver fast, non-DOM UI for demanding surfaces. This crate is the foundation for WGPUI-based apps like Autopilot GUI.
