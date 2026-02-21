# OpenAgents Crates

This directory contains the Rust crates that power OpenAgents. Each section below provides a
single-paragraph overview of a crate to explain its role in the workspace.

## Workspace Layout Plan (Draft)
This plan keeps a single Cargo workspace but groups crates by purpose so product code stays
separate from shared platform, core, and UI code. The UI group should live under `crates/ui/`
(not `crates/ux/`). The Nostr crates are already nested under `crates/nostr/`.

Proposed grouping (adjust as needed):
- `crates/products/`: autopilot, pylon
- `crates/app-core/`: autopilot_app, autopilot_ui
- `crates/core/`: adjutant, autopilot-core, dsrs, dsrs-macros, rlm, frlm
- `crates/platform/`: agent, protocol, runtime, relay, compute, gateway, lm-router,
  local-inference, nostr (core + client), issues, openagents-utils
- `crates/ui/`: wgpui, editor, vim
- `crates/integrations/`: gpt-oss, codex-client, codex-mcp, ai-server, spark, voice
- `crates/tools/`: arrow, testing, ws-test

Proposed steps:
1. Create the group folders under `crates/` and move the crate directories.
2. Update the root `Cargo.toml` workspace members to include the new paths (glob per group).
3. Update all path dependencies in crate `Cargo.toml` files to the new locations.
4. Refresh docs and tooling references (this file, any build scripts, CI paths).
5. Run `cargo check` and fix any path or feature fallout.

Note: Onyx is now an app surface at `apps/onyx/` (moved from `crates/onyx` on 2026-02-20).

## adjutant
The adjutant crate is the autonomous task execution engine. It plans and routes work via DSPy
decision pipelines, delegates to Codex or RLM when needed, runs tool execution, and records session
outcomes for self-improvement and optimization.

## agent
The agent crate defines the core data model for sovereign agents: configs, lifecycle states, spawn
requests, and registry persistence. It owns identity and wallet metadata but does not execute
agents itself.

## ai-server
The ai-server crate manages the local AI Gateway sidecar (Bun/JS), including dependency install,
process lifecycle, port checks, and health/analytics helpers used by desktop flows.

## arrow
The arrow crate provides happy-path testing utilities for Autopilot, including scenario-driven
test helpers and structured assertions for end-to-end flows.

## autopilot
The autopilot crate is the GPU-accelerated terminal UI for Autopilot. It renders streaming
Markdown, tool cards, panels, and command palettes, manages workspace/session history, and drives
Codex app-server runs and Adjutant autopilot loops.

## autopilot-core
The autopilot-core crate provides shared execution logic for Autopilot, including DSPy workflows,
checkpointing, preflight checks, session logging, and streaming event normalization.

## autopilot_app
The autopilot_app crate is the shared app core for Autopilot CLI and desktop surfaces: it owns
workspace/session handles, dispatches user actions, and emits a stream of app events for UI layers.

## autopilot_ui
The autopilot_ui crate hosts shared WGPUI surfaces for Autopilot, including immediate-mode views,
thread panels, and desktop scaffolding used by the native desktop host.

## codex-client
The codex-client crate is a Rust client for the Codex app-server JSON-RPC API, with streaming
support and typed request/response helpers.

## codex-mcp
The codex-mcp crate provides minimal MCP (Model Context Protocol) JSON-RPC helpers for Codex-style
stdio servers, focused on the `tools/list` and `tools/call` flow.

## compute
The compute crate implements a NIP-90 DVM provider that sells compute on Nostr. It bids on jobs,
executes inference or agent backends, and publishes results back to relays.

## dsrs
The dsrs crate is the Rust implementation of DSPy (Declarative Self-improving Programming). It
provides signatures, predictors, optimizers, retrieval, tracing, and evaluation for declarative
agent programming.

## dsrs-macros
The dsrs-macros crate provides procedural macros for declaring DSPy signatures and optimizable
modules in Rust.

## editor
The editor crate is a WGPUI-based text editor component built on Ropey and Tree-sitter, providing
editable buffers, caret/selection logic, and syntax highlighting.

## frlm
The frlm crate implements Federated Recursive Language Models, orchestrating distributed subqueries
across local and swarm backends with trace emission and verification.

## gateway
The gateway crate provides a unified abstraction layer between agents and external AI service
providers, including authentication, health checks, and capability discovery.

## gpt-oss
The gpt-oss crate is a Rust client for the GPT-OSS Responses API with streaming support and Harmony
prompt helpers.

## issues
The issues crate provides the SQLite-backed issue tracking core with migrations and lifecycle
transitions used by Autopilot and CLI tooling.

## lm-router
The lm-router crate routes LLM calls across multiple backends with usage tracking and model routing.

## local-inference
The local-inference crate defines the `LocalModelBackend` trait and shared request/response types
for local inference engines, enabling OpenAgents to swap GPT-OSS, fm-bridge, or custom backends via
a consistent API.

## nostr (core)
The nostr core crate (`crates/nostr/core`) provides the foundational Nostr protocol types,
serialization, and crypto helpers used across OpenAgents.

## nostr-client
The nostr-client crate (`crates/nostr/client`) provides relay client APIs built on top of the Nostr
core crate.

## openagents-relay (relay)
The relay crate defines the relay protocol message types shared by browser/worker/tunnel clients.

## runtime (runtime)
The runtime crate provides a pluggable execution environment for agents, including filesystem-like
mounts, containers, identity, and telemetry hooks.

## openagents-spark (spark)
The spark crate integrates Breez Spark payments for OpenAgents wallets and compute flows.

## openagents-utils
The openagents-utils crate provides shared utility helpers used across the workspace.

## openagents-proto
The openagents-proto crate provides generated Rust wire contracts from `proto/openagents/*/v1/*`
and explicit wire-to-domain boundary examples used by Rust services and clients.

## protocol
The protocol crate provides typed job schemas with deterministic hashing, provenance, and
verification strategies for the OpenAgents swarm.

## pylon
The pylon crate is the local runtime for sovereign agents. It supports host mode for running agents
and provider mode for selling NIP-90 compute, with relay and wallet integration.

## rlm
The rlm crate is the Recursive Language Model execution engine, with tools for long-context
analysis and optional DSPy integration.

## testing
The testing crate provides ADR compliance checks and meta-coverage tests for the OpenAgents
workspace.

## vim
The vim crate is an editor-agnostic Vim emulation layer with mode, motion, and operator handling.

## voice
The voice crate provides Whisper-based speech-to-text transcription utilities.

## wgpui
The wgpui crate is the GPU-accelerated UI library used across desktop and web, providing layout,
text/Markdown rendering, and input handling.

## ws-test
The ws-test crate is a local WebSocket test server for Hyperion integration work.
